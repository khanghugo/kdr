use super::pp_trait::{PostProcessingModule, PostProcessingPipeline};

pub struct Kuwahara {
    pipeline: PostProcessingPipeline,
}

impl PostProcessingModule for Kuwahara {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("./kuwahara.wgsl"))
    }

    fn post_processing_effect_name() -> &'static str {
        "kuwahara filter"
    }

    fn get_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.pipeline.bind_group_layout
    }

    fn get_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline.pipeline
    }

    fn get_sampler(&self) -> &wgpu::Sampler {
        &self.pipeline.sampler
    }

    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &crate::renderer::utils::FullScrenTriVertexShader,
    ) -> Self {
        let pipeline = Self::create_pipeline(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        );

        Self { pipeline }
    }
}
