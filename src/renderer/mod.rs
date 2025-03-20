use std::sync::Arc;

use bsp_buffer::{BspBuffer, BspLoader};
use camera::Camera;
use mdl_buffer::{MdlBuffer, MdlLoader};
use wgpu::Extent3d;
use winit::window::Window;

pub mod bsp_buffer;
pub mod bsp_lightmap;
pub mod camera;
pub mod mdl_buffer;
pub mod oit;
pub mod texture_buffer;
pub mod utils;
pub mod vertex_buffer;

pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub bsp_render_pipeline: wgpu::RenderPipeline,
    pub mdl_render_pipeline: wgpu::RenderPipeline,
    pub swapchain_format: wgpu::TextureFormat,
    pub surface: wgpu::Surface<'static>,
    pub cam_buffer: wgpu::Buffer,
    pub cam_bind_group: wgpu::BindGroup,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        self.device.destroy();
        self.cam_buffer.destroy();
        self.depth_texture.destroy();
    }
}

pub struct RenderState {
    // can load multiple bsp
    pub bsp_buffers: Vec<BspBuffer>,
    pub mdl_buffers: Vec<MdlBuffer>,

    pub camera: Camera,

    // debug
    pub draw_call: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            camera: Default::default(),
            bsp_buffers: vec![],
            mdl_buffers: vec![],
            draw_call: 0,
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
        limits.max_storage_buffer_binding_size = 4096 * 4;
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

        // camera stuffs
        let cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera buffer"),
            size: 4 * 4 * 4, // 4x4 matrix
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false, // we will update it
        });

        let cam_bind_group_layout =
            device.create_bind_group_layout(&Camera::bind_group_layout_descriptor());

        // should go into the camera function
        let cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &cam_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cam_buffer.as_entire_binding(),
            }],
        });

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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // rendering stuffs
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        // enable alpha blending
        let alpha_blending = wgpu::ColorTargetState {
            format: swapchain_format,
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

        let fragment_targets = vec![alpha_blending];

        // bsp render pipeline
        let bsp_render_pipeline =
            BspLoader::create_render_pipeline_opaque(&device, fragment_targets.clone());

        // mdl render pipeline
        let mdl_render_pipeline = MdlLoader::create_render_pipeline(&device, fragment_targets);

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            present_mode: wgpu::PresentMode::Immediate, // to mailbox later
            ..config
        };

        surface.configure(&device, &config);

        Self {
            device,
            queue,
            bsp_render_pipeline,
            mdl_render_pipeline,
            swapchain_format,
            surface,
            cam_bind_group,
            cam_buffer,
            depth_texture,
            depth_view,
        }
    }

    pub fn render(&self, state: &mut RenderState) {
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // camera projection
        // there is no need to do model-view projection for bsp
        // so we save the number here and we will do the matrix for the model
        let view_proj = state.camera.build_view_projection_matrix();

        {
            let view_proj_cast: &[f32; 16] = view_proj.as_ref();
            let view_proj_bytes: &[u8] = bytemuck::cast_slice(view_proj_cast);
            self.queue
                .write_buffer(&self.cam_buffer, 0, view_proj_bytes);
        }

        // render bsp
        {
            let pass_descriptor = wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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

            let mut rpass = encoder.begin_render_pass(&pass_descriptor);

            state.draw_call = 0;

            // drawing bsp
            {
                rpass.set_pipeline(&self.bsp_render_pipeline);
                rpass.set_bind_group(0, &self.cam_bind_group, &[]);

                state.bsp_buffers.iter().for_each(|bsp_buffer| {
                    rpass.set_bind_group(2, &bsp_buffer.lightmap.bind_group, &[]);

                    let opaque_buffer = &bsp_buffer.opaque;

                    opaque_buffer.iter().for_each(|batch| {
                        state.draw_call += 1;

                        rpass.set_bind_group(
                            1,
                            &bsp_buffer.textures[batch.texture_array_index].bind_group,
                            &[],
                        );
                        rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            batch.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                    });

                    let transparent_buffer = &bsp_buffer.transparent;

                    // drawing entities
                    transparent_buffer.iter().for_each(|batch| {
                        state.draw_call += 1;

                        rpass.set_bind_group(
                            1,
                            &bsp_buffer.textures[batch.texture_array_index].bind_group,
                            &[],
                        );
                        rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            batch.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                    });
                });
            }

            // drawing mdl
            {
                rpass.set_pipeline(&self.mdl_render_pipeline);
                rpass.set_bind_group(0, &self.cam_bind_group, &[]);

                state.mdl_buffers.iter().for_each(|mdl_buffer| {
                    // model projection
                    {
                        rpass.set_bind_group(2, &mdl_buffer.mvps.bind_group, &[]);
                        // let buf = mdl_buffer.mvps.entity_infos.
                    }

                    mdl_buffer.vertices.iter().for_each(|batch| {
                        state.draw_call += 1;

                        rpass.set_bind_group(
                            1,
                            &mdl_buffer.textures[batch.texture_array_idx as usize].bind_group,
                            &[],
                        );
                        rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            batch.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                    });
                });
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
