use std::sync::Arc;

use camera::{Camera, CameraBuffer};
use oit::{OITRenderTarget, OITResolver};
use wgpu::Extent3d;
use winit::window::Window;
use world_buffer::{WorldBuffer, WorldLoader};

pub mod bsp_lightmap;
pub mod camera;
pub mod mvp_buffer;
pub mod oit;
pub mod texture_buffer;
pub mod utils;
pub mod world_buffer;

pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub world_opaque_render_pipeline: wgpu::RenderPipeline,
    pub world_transparent_render_pipeline: wgpu::RenderPipeline,
    pub swapchain_format: wgpu::TextureFormat,
    pub surface: wgpu::Surface<'static>,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub oit_resolver: OITResolver,
    pub camera_buffer: CameraBuffer,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        self.device.destroy();
        self.depth_texture.destroy();
    }
}

pub struct RenderState {
    pub world_buffer: Vec<WorldBuffer>,

    pub camera: Camera,

    // debug
    pub draw_call: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            camera: Default::default(),
            draw_call: 0,
            world_buffer: vec![],
        }
    }
}

impl RenderContext {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        // edit limits
        let mut limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        limits.max_texture_array_layers = 1024;
        limits.max_storage_buffers_per_shader_stage = 4;
        // this is for mvp matrices
        limits.max_storage_buffer_binding_size = (4 * 4 * 4) // 1 matrix4x4f
            * 1024 // 1000 entities
        ;
        // end limits

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::TEXTURE_BINDING_ARRAY,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .unwrap();

        // camera buffer
        let camera_buffer = CameraBuffer::create(&device);

        // depth stuffs
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // rendering stuffs
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        // opaque pass and then transparent passs
        let opaque_blending = wgpu::ColorTargetState {
            format: swapchain_format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };

        let transparent_blending = OITRenderTarget::targets();

        let world_opaque_render_pipeline =
            WorldLoader::create_render_pipeline(&device, vec![opaque_blending], true);
        let world_transparent_render_pipeline =
            WorldLoader::create_render_pipeline(&device, transparent_blending.into(), false);

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            present_mode: wgpu::PresentMode::Immediate, // to mailbox later
            ..config
        };

        let oit_resolver = OITResolver::new(&device, &config);

        surface.configure(&device, &config);

        Self {
            device,
            queue,
            swapchain_format,
            surface,
            depth_texture,
            depth_view,
            world_opaque_render_pipeline,
            world_transparent_render_pipeline,
            oit_resolver,
            camera_buffer,
        }
    }

    pub fn render(&self, state: &mut RenderState) {
        let frame = self.surface.get_current_texture().unwrap();
        let surface_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // update camera buffer
        {
            let view = state.camera.view();
            let view_cast: &[f32; 16] = view.as_ref();
            let view_bytes: &[u8] = bytemuck::cast_slice(view_cast);

            let proj = state.camera.proj();
            let proj_cast: &[f32; 16] = proj.as_ref();
            let proj_bytes: &[u8] = bytemuck::cast_slice(proj_cast);

            let pos = state.camera.pos;
            let pos_cast: &[f32; 3] = pos.as_ref();
            let pos_bytes: &[u8] = bytemuck::cast_slice(pos_cast);

            self.queue
                .write_buffer(&self.camera_buffer.view, 0, view_bytes);
            self.queue
                .write_buffer(&self.camera_buffer.projection, 0, proj_bytes);
            self.queue
                .write_buffer(&self.camera_buffer.position, 0, pos_bytes);
        }

        state.draw_call = 0;

        // world opaque pass
        if true {
            let opaque_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("world opaque pass descriptor"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            let mut opaque_pass = encoder.begin_render_pass(&opaque_pass_descriptor);

            opaque_pass.set_pipeline(&self.world_opaque_render_pipeline);
            opaque_pass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);

            state.world_buffer.iter().for_each(|world_buffer| {
                opaque_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);
                opaque_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);

                world_buffer.opaque.iter().for_each(|batch| {
                    state.draw_call += 1;

                    // texture array
                    opaque_pass.set_bind_group(
                        2,
                        &world_buffer.textures[batch.texture_array_index].bind_group,
                        &[],
                    );

                    opaque_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    opaque_pass
                        .set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    opaque_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                });
            });
        }

        // world transparent pass
        if true {
            let transparent_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("world transparent pass descriptor"),
                color_attachments: &self.oit_resolver.render_pass_color_attachments(),
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            let mut transparent_pass = encoder.begin_render_pass(&transparent_pass_descriptor);

            transparent_pass.set_pipeline(&self.world_transparent_render_pipeline);
            transparent_pass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);

            state.world_buffer.iter().for_each(|world_buffer| {
                transparent_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);
                transparent_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);

                world_buffer.transparent.iter().for_each(|batch| {
                    state.draw_call += 1;

                    // texture array
                    transparent_pass.set_bind_group(
                        2,
                        &world_buffer.textures[batch.texture_array_index].bind_group,
                        &[],
                    );

                    transparent_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    transparent_pass
                        .set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    transparent_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                });
            });
        }

        // resolve pass
        if true {
            self.oit_resolver.resolve(&mut encoder, &surface_view);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
