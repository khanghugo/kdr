use crate::renderer::utils::FullScrenTriVertexShader;

use super::{PostProcessingModule, PostProcessingPipeline};

pub struct GrayScale {
    pipeline: PostProcessingPipeline,
}

impl PostProcessingModule for GrayScale {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("../shader/gray_scale.wgsl"))
    }
    fn post_processing_effect_name() -> &'static str {
        "gray scale"
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
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
    ) -> Self {
        let pipeline =
            Self::create_pipeline(device, input_texture_format, fullscreen_tri_vertex_shader);

        Self { pipeline }
    }
}
