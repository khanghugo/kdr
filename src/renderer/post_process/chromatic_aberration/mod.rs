use std::sync::Arc;

use super::pp_trait::{PostProcessingModule, PostProcessingPipeline};

pub struct ChromaticAberration {
    pipeline: PostProcessingPipeline,
    depth_texture: Arc<wgpu::Texture>,
}

impl PostProcessingModule for ChromaticAberration {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("./chromatic_aberration.wgsl"))
    }

    fn post_processing_effect_name() -> &'static str {
        "chromatic aberration"
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

    fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Self::bind_group_layout_descriptor_label(),
            entries: &[
                // scene texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        }
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        input_texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Self::bind_group_label(),
            layout: &self.get_bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .depth_texture
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(self.get_sampler()),
                },
            ],
        })
    }

    fn new(
        device: &wgpu::Device,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &crate::renderer::utils::FullScrenTriVertexShader,
    ) -> Self {
        panic!("dont use this method")
    }
}

impl ChromaticAberration {
    pub fn new2(
        device: &wgpu::Device,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &crate::renderer::utils::FullScrenTriVertexShader,
        depth_texture: Arc<wgpu::Texture>,
    ) -> Self {
        let pipeline =
            Self::create_pipeline(device, input_texture_format, fullscreen_tri_vertex_shader);

        Self {
            pipeline,
            depth_texture,
        }
    }
}
