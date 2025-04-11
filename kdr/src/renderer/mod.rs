use std::sync::Arc;

use camera::{Camera, CameraBuffer};
use finalize::FinalizeRenderPipeline;
use oit::{OITRenderTarget, OITResolver};
use post_process::PostProcessing;
use render_targets::RenderTargets;
use skybox::{SkyboxBuffer, SkyboxLoader};
use utils::FullScrenTriVertexShader;
use winit::window::Window;
use world_buffer::{WorldBuffer, WorldLoader, WorldPushConstants};

// need this to have window.canvas()
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use crate::app::constants::MAX_MVP;

pub mod bsp_lightmap;
pub mod camera;
pub mod egui_renderer;
pub mod finalize;
pub mod mvp_buffer;
pub mod oit;
pub mod post_process;
mod render_targets;
pub mod skybox;
pub mod texture_buffer;
pub mod utils;
pub mod world_buffer;

pub struct RenderContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    world_z_prepass_render_pipeline: wgpu::RenderPipeline,
    world_opaque_render_pipeline: wgpu::RenderPipeline,
    world_skybox_mask_render_pipeline: wgpu::RenderPipeline,
    world_transparent_render_pipeline: wgpu::RenderPipeline,
    swapchain_format: wgpu::TextureFormat,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    oit_resolver: OITResolver,
    camera_buffer: CameraBuffer,
    render_targets: RenderTargets,
    finalize_render_pipeline: FinalizeRenderPipeline,
    fullscreen_tri_vertex_shader: FullScrenTriVertexShader,
    pub post_processing: PostProcessing,
    pub skybox_loader: SkyboxLoader,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        self.device.destroy();
    }
}

pub struct RenderState {
    pub world_buffer: Vec<WorldBuffer>,
    pub skybox: Option<SkyboxBuffer>,

    pub camera: Camera,
    pub render_options: RenderOptions,

    // debug
    pub draw_call: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            camera: Default::default(),
            skybox: None,
            draw_call: 0,
            world_buffer: vec![],
            render_options: RenderOptions::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct RenderOptions {
    pub render_nodraw: bool,
    // TODO, eh, make it better?
    pub render_beyond_sky: bool,
    pub render_skybox: bool,
    pub render_transparent: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            render_nodraw: false,
            render_beyond_sky: false,
            render_skybox: true,
            render_transparent: true,
        }
    }
}

impl RenderContext {
    pub async fn new(window: Arc<Window>) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let size = window.inner_size();

        #[cfg(target_arch = "wasm32")]
        let size = {
            // for some fucking reasons it has to be like this fuckinghell
            let canvas = window.canvas().unwrap();
            winit::dpi::LogicalSize::new(canvas.width(), canvas.height())
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN // native windows/linux
            | wgpu::Backends::GL, // webgpu doesnt work well on modern browsers, yet TODO: come back in 2 years
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions {
                // need to be explicit here just to be safe
                gl: wgpu::GlBackendOptions {
                    gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
                },
                dx12: wgpu::Dx12BackendOptions::default(),
            },
        });

        let surface = instance.create_surface(window).unwrap();

        // for some FUCKING reasons, this woks but specifically using a ReqestAdapterOptions doesn's work.
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
            .await
            .unwrap();

        // let adapter = instance
        //     .request_adapter(&wgpu::RequestAdapterOptions {
        //         power_preference: wgpu::PowerPreference::HighPerformance,
        //         force_fallback_adapter: true,
        //         compatible_surface: Some(&surface),
        //     })
        //     .block_on()
        //     .unwrap();

        // edit limits
        let mut limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        limits.max_texture_array_layers = 512;
        // this is for mvp matrices
        limits.max_uniform_buffer_binding_size = (4 * 4 * 4) // 1 matrix4x4f
            * MAX_MVP; // 512 entities at 32.8 KB
        limits.max_push_constant_size = 128; // TODO may not be working
        // end limits

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::DEPTH32FLOAT_STENCIL8
                        | wgpu::Features::PUSH_CONSTANTS,
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
        let depth_texture_format = RenderTargets::depth_texture_format();

        let world_z_prepass_render_pipeline =
            WorldLoader::create_z_prepass_render_pipeline(&device, vec![], depth_texture_format);
        let world_opaque_render_pipeline = WorldLoader::create_opaque_render_pipeline(
            &device,
            vec![opaque_blending.clone()],
            depth_texture_format,
        );
        let world_skybox_mask_render_pipeline =
            WorldLoader::create_skybox_mask_render_pipeline(&device, vec![], depth_texture_format);
        let world_transparent_render_pipeline = WorldLoader::create_transparent_render_pipeline(
            &device,
            transparent_blending.into(),
            depth_texture_format,
        );

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            present_mode: wgpu::PresentMode::Fifo,
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

        let post_processing = PostProcessing::create_pipelines(
            &device,
            size.width,
            size.height,
            render_target_format,
            &fullscreen_tri_vertex_shader,
            render_targets.depth_texture.clone(),
        );

        let skybox_render_pipeline = SkyboxLoader::create_render_pipeline(
            &device,
            vec![opaque_blending],
            depth_texture_format,
        );

        Self {
            device,
            queue,
            swapchain_format,
            surface,
            surface_config: config,
            world_z_prepass_render_pipeline,
            world_opaque_render_pipeline,
            world_skybox_mask_render_pipeline,
            world_transparent_render_pipeline,
            oit_resolver,
            camera_buffer,
            render_targets,
            finalize_render_pipeline,
            fullscreen_tri_vertex_shader,
            post_processing,
            skybox_loader: skybox_render_pipeline,
        }
    }

    pub fn render(
        &mut self,
        state: &mut RenderState,
        encoder: &mut wgpu::CommandEncoder,
        swapchain_view: &wgpu::TextureView,
    ) {
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

        // UPDATE: no more z pre pass, it is more troubling than it is worth it
        // the game doesn't have enough polygon to worry about overdrawing
        // on top of that, dealing with alpha test texture is not very fun
        // it might hurt more performance just to fix the alpha test texture depth
        //
        // z prepass
        // if true {
        //     let z_prepass_pass_descriptor = wgpu::RenderPassDescriptor {
        //         label: Some("world z prepass pass descriptor"),
        //         color_attachments: &[],
        //         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
        //             view: &self.render_targets.depth_view,
        //             depth_ops: Some(wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(1.0),
        //                 store: wgpu::StoreOp::Store,
        //             }),
        //             stencil_ops: None,
        //         }),
        //         timestamp_writes: None,
        //         occlusion_query_set: None,
        //     };

        //     let mut z_prepass_pass = encoder.begin_render_pass(&z_prepass_pass_descriptor);

        //     z_prepass_pass.set_pipeline(&self.world_z_prepass_render_pipeline);
        //     z_prepass_pass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);

        //     state.world_buffer.iter().for_each(|world_buffer| {
        //         z_prepass_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);
        //         z_prepass_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);

        //         world_buffer.opaque.iter().for_each(|batch| {
        //             // state.draw_call += 1;

        //             // texture array
        //             z_prepass_pass.set_bind_group(
        //                 2,
        //                 &world_buffer.textures[batch.texture_array_index].bind_group,
        //                 &[],
        //             );

        //             z_prepass_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
        //             z_prepass_pass
        //                 .set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        //             z_prepass_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
        //         });
        //     });
        // }

        let world_push_constants = WorldPushConstants {
            render_nodraw: if state.render_options.render_nodraw {
                1
            } else {
                0
            },
        };

        let push_data = bytemuck::bytes_of(&world_push_constants);

        // world opaque pass
        if true {
            let opaque_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("world opaque pass descriptor"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.main_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.render_targets.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    // need to clear stencils here because skybox mask doesn't write over it
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            let mut opaque_pass = encoder.begin_render_pass(&opaque_pass_descriptor);
            opaque_pass.set_pipeline(&self.world_opaque_render_pipeline);

            // there are two set_push_constants method. WTF?
            opaque_pass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, push_data);

            state.world_buffer.iter().for_each(|world_buffer| {
                opaque_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);
                opaque_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);

                world_buffer.opaque.iter().for_each(|batch| {
                    state.draw_call += 1;

                    opaque_pass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);

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

        // skybox mask
        if state.render_options.render_skybox {
            state.world_buffer.iter().for_each(|world_buffer| {
                let Some(batch_idx) = world_buffer.skybrush_batch_index else {
                    return;
                };
                let batch = &world_buffer.opaque[batch_idx];

                let skybox_mask_pass_descriptor = wgpu::RenderPassDescriptor {
                    label: Some("world skybox mask pass descriptor"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.render_targets.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: Some(wgpu::Operations {
                            // even though this step has "Clear", it can't clear stencil
                            // need to clear stencil in skybox pass step
                            load: wgpu::LoadOp::Clear(0),
                            store: wgpu::StoreOp::Store,
                        }),
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                };

                let mut rpass = encoder.begin_render_pass(&skybox_mask_pass_descriptor);

                // VERY IMPORTANT
                rpass.set_stencil_reference(1);

                rpass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);
                rpass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);

                rpass.set_pipeline(&self.world_skybox_mask_render_pipeline);
                rpass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);

                rpass.set_bind_group(
                    2,
                    &world_buffer.textures[batch.texture_array_index].bind_group,
                    &[],
                );

                rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
            });
        }

        // skybox pass
        if state.render_options.render_skybox {
            if let Some(ref skybox_buffer) = state.skybox {
                let skybox_pass_descriptor = wgpu::RenderPassDescriptor {
                    label: Some("skybox pass descriptor"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.render_targets.main_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // load previously written opaque
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.render_targets.depth_view,
                        depth_ops: None,
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                };

                let mut rpass = encoder.begin_render_pass(&skybox_pass_descriptor);

                rpass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);
                rpass.set_bind_group(1, &skybox_buffer.bind_group, &[]);

                rpass.set_pipeline(&self.skybox_loader.pipeline);
                // VERY IMPORTANT
                rpass.set_stencil_reference(1);

                rpass.set_vertex_buffer(0, skybox_buffer.vertex_buffer.slice(..));
                rpass.set_index_buffer(
                    skybox_buffer.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                rpass.draw_indexed(0..skybox_buffer.index_count, 0, 0..1);
            }
        }

        // world transparent pass
        // if resolve pass runs but this pass does not, the result image is black
        // UPDATE, fake news, can skip this and resolve
        if state.render_options.render_transparent {
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
                transparent_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);
                transparent_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);

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
        if state.render_options.render_transparent {
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
            self.post_processing.run_post_processing_effects(
                &self.device,
                encoder,
                &self.render_targets.main_texture,
                &self.render_targets.composite_texture,
            );
        }

        // writes to surface view because simply blitting doesn's work
        // surface texture does not have COPY_DST
        {
            self.finalize_render_pipeline
                .finalize_to_swapchain(encoder, &swapchain_view);
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.surface_config
    }

    pub fn surface_texture(&self) -> wgpu::SurfaceTexture {
        self.surface
            .get_current_texture()
            .expect("cannot get surface texture")
    }

    pub fn swapchain_format(&self) -> &wgpu::TextureFormat {
        &self.swapchain_format
    }
}
