use std::sync::{Arc, RwLock};

use camera::CameraBuffer;
use finalize::FinalizeRenderPipeline;
use oit::{OITRenderTarget, OITResolver};
use post_process::PostProcessing;
use render_targets::RenderTargets;
use skybox::SkyboxLoader;
use utils::FullScrenTriVertexShader;
use winit::window::Window;
use world_buffer::WorldLoader;

// need this to have window.canvas()
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

#[allow(unused_imports)]
use crate::app::constants::{DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_MVP};

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
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub world_z_prepass_render_pipeline: wgpu::RenderPipeline,
    pub world_opaque_render_pipeline: wgpu::RenderPipeline,
    pub world_skybox_mask_render_pipeline: wgpu::RenderPipeline,
    pub world_transparent_render_pipeline: wgpu::RenderPipeline,
    pub swapchain_format: wgpu::TextureFormat,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub oit_resolver: OITResolver,
    pub camera_buffer: CameraBuffer,
    pub render_targets: RenderTargets,
    pub finalize_render_pipeline: FinalizeRenderPipeline,
    pub post_processing: Arc<RwLock<PostProcessing>>,
    pub skybox_loader: SkyboxLoader,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        self.device.destroy();
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
            winit::dpi::LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)
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
        limits.max_texture_array_layers = 1024;
        // this is for mvp matrices
        limits.max_uniform_buffer_binding_size = (4 * 4 * 4) // 1 matrix4x4f
            * MAX_MVP as u32; // 1024 entities at 64 KB aka max size
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

        let post_processing = Arc::new(RwLock::new(PostProcessing::create_pipelines(
            &device,
            &queue,
            size.width,
            size.height,
            render_target_format,
            &fullscreen_tri_vertex_shader,
            render_targets.depth_texture.clone(),
        )));

        // this one doesnt need to be in mutex, but whatever
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
            post_processing,
            skybox_loader: skybox_render_pipeline,
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

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        let new_render_targets = RenderTargets::new(&self.device, width, height);

        // make sure anything using render targets need to update their bind group
        self.oit_resolver.resize(&self.device, width, height);

        self.finalize_render_pipeline.bind_group = FinalizeRenderPipeline::create_bind_group(
            &self.device,
            &self.finalize_render_pipeline.bind_group_layout,
            &new_render_targets.composite_view,
            &self.finalize_render_pipeline.sampler,
        );

        self.post_processing
            .write()
            .unwrap()
            .resize(&self.device, width, height);

        self.render_targets = new_render_targets;
    }
}
