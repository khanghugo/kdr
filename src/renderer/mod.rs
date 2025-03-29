use std::sync::Arc;

use camera::{Camera, CameraBuffer};
use finalize::FinalizeRenderPipeline;
use oit::{OITRenderTarget, OITResolver};
use post_process::PostProcessing;
use utils::FullScrenTriVertexShader;
use wgpu::Extent3d;
use winit::window::Window;
use world_buffer::{WorldBuffer, WorldLoader};

pub mod bsp_lightmap;
pub mod camera;
pub mod finalize;
pub mod mvp_buffer;
pub mod oit;
pub mod post_process;
pub mod texture_buffer;
pub mod utils;
pub mod world_buffer;

pub struct RenderTargets {
    main_texture: wgpu::Texture,
    main_view: wgpu::TextureView,
    // need a composite texture because wgpu cannot sample a texture while writing on it
    // it happens when we want to do post processing, we have to read the current image and then write on it
    composite_texture: wgpu::Texture,
    composite_view: wgpu::TextureView,
    depth_texture: Arc<wgpu::Texture>,
    depth_view: wgpu::TextureView,
}

impl RenderTargets {
    fn main_texture_format() -> wgpu::TextureFormat {
        wgpu::TextureFormat::Rgba16Float
        // wgpu::TextureFormat::Bgra8Unorm
    }

    fn composite_texture_format() -> wgpu::TextureFormat {
        Self::main_texture_format()
    }

    fn depth_texture_format() -> wgpu::TextureFormat {
        wgpu::TextureFormat::Depth32Float
    }

    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let main_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("main render texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::main_texture_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let main_view = main_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // depth stuffs
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::depth_texture_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let composite_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("composite texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::main_texture_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let composite_view = composite_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            main_texture,
            main_view,
            depth_texture,
            depth_view,
            composite_texture,
            composite_view,
        }
    }
}

impl Drop for RenderTargets {
    fn drop(&mut self) {
        self.main_texture.destroy();
        self.depth_texture.destroy();
        self.composite_texture.destroy();
    }
}

pub struct RenderContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    world_opaque_render_pipeline: wgpu::RenderPipeline,
    world_transparent_render_pipeline: wgpu::RenderPipeline,
    swapchain_format: wgpu::TextureFormat,
    surface: wgpu::Surface<'static>,
    oit_resolver: OITResolver,
    camera_buffer: CameraBuffer,
    render_targets: RenderTargets,
    finalize_render_pipeline: FinalizeRenderPipeline,
    fullscreen_tri_vertex_shader: FullScrenTriVertexShader,
    post_processing: PostProcessing,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        self.device.destroy();
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
                    required_features: wgpu::Features::TEXTURE_BINDING_ARRAY
                        | wgpu::Features::TIMESTAMP_QUERY,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .unwrap();

        // camera buffer
        let camera_buffer = CameraBuffer::create(&device);

        // swap chain stuffs
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        let render_targets = RenderTargets::new(&device, size.width, size.height);
        let render_target_format = RenderTargets::main_texture_format();

        // common shader
        let fullscreen_tri_vertex_shader = FullScrenTriVertexShader::create_shader_module(&device);

        // opaque pass and then transparent passs
        let opaque_blending = wgpu::ColorTargetState {
            format: render_target_format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };

        let _alpha_blending = wgpu::ColorTargetState {
            format: render_target_format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
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
            present_mode: wgpu::PresentMode::Mailbox, // to mailbox later
            ..config
        };

        let oit_resolver = OITResolver::new(
            &device,
            size.width,
            size.height,
            render_target_format,
            &fullscreen_tri_vertex_shader,
        );

        surface.configure(&device, &config);

        let finalize_render_pipeline = FinalizeRenderPipeline::create_pipeline(
            &device,
            // take in composite view and then render it out to the target swapchain
            // this means composite step is required to move main to composite
            &render_targets.composite_view,
            swapchain_format,
            &fullscreen_tri_vertex_shader,
        );

        // let profiler = GpuProfiler::new(
        //     &device,
        //     GpuProfilerSettings {
        //         enable_timer_queries: true,
        //         enable_debug_groups: true,
        //         ..Default::default()
        //     },
        // )
        // .unwrap();
        //

        let post_processing = PostProcessing::create_pipelines(
            &device,
            size.width,
            size.height,
            render_target_format,
            &fullscreen_tri_vertex_shader,
        );

        Self {
            device,
            queue,
            swapchain_format,
            surface,
            world_opaque_render_pipeline,
            world_transparent_render_pipeline,
            oit_resolver,
            camera_buffer,
            render_targets,
            finalize_render_pipeline,
            fullscreen_tri_vertex_shader,
            post_processing,
        }
    }

    pub fn render(&mut self, state: &mut RenderState) {
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
                    view: &self.render_targets.main_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.render_targets.depth_view,
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
                    view: &self.render_targets.depth_view,
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

        // oit resolve
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("oit resolve pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.main_view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    resolve_target: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.oit_resolver.composite(&mut rpass);
        }

        // post processing
        // must be enabled because finalize is sending composite to swapchain
        // composite is empty at this moment
        {
            self.post_processing.execute(
                &self.device,
                &mut encoder,
                &self.render_targets.main_texture,
                &self.render_targets.composite_texture,
            );
        }

        let swapchain_surface_texture = self.surface.get_current_texture().unwrap();

        // blit to surface view
        {
            let swapchain_view = swapchain_surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.finalize_render_pipeline
                .finalize_to_swapchain(&mut encoder, &swapchain_view);
        }

        self.queue.submit(Some(encoder.finish()));
        swapchain_surface_texture.present();
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
