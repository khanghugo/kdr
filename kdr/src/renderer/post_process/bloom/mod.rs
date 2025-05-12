use crate::renderer::utils::FullScrenTriVertexShader;

use super::pp_trait::{PostProcessingModule, PostProcessingPipeline};

pub struct BrightnessExtraction {
    pipeline: PostProcessingPipeline,
}

impl PostProcessingModule for BrightnessExtraction {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("./brightness_extraction.wgsl"))
    }

    fn post_processing_effect_name() -> &'static str {
        "brightness extraction"
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
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
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

pub struct KawaseBlur {
    pipeline: PostProcessingPipeline,
}

impl PostProcessingModule for KawaseBlur {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("./kawase_blur.wgsl"))
    }

    fn post_processing_effect_name() -> &'static str {
        "kawase blur"
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
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
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

pub struct Bloom {
    brightness_extraction_pipeline: BrightnessExtraction,
    kawase_blur_pipeline: KawaseBlur,
    composite_pipeline: PostProcessingPipeline,
    bloom_texture: wgpu::Texture,
    input_texture_format: wgpu::TextureFormat,
}

impl Drop for Bloom {
    fn drop(&mut self) {
        self.bloom_texture.destroy();
    }
}

impl PostProcessingModule for Bloom {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("./composite.wgsl"))
    }

    fn post_processing_effect_name() -> &'static str {
        "bloom"
    }

    fn get_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.composite_pipeline.bind_group_layout
    }

    fn get_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.composite_pipeline.pipeline
    }

    fn get_sampler(&self) -> &wgpu::Sampler {
        &self.composite_pipeline.sampler
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
                // bloom texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
                            .bloom_texture
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
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _input_texture_format: wgpu::TextureFormat,
        _fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
    ) -> Self {
        panic!("this shouldn't be used")
    }
}

impl Bloom {
    pub fn create_bloom_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        input_texture_format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: input_texture_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
            label: Some("bloom texture"),
        })
    }
    /// Use this one instead of `Bloom::new()`
    pub fn new2(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
        width: u32,
        height: u32,
    ) -> Self {
        let brightness_extraction_pipeline = BrightnessExtraction::new(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        );
        let kawase_blur_pipeline = KawaseBlur::new(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        );
        let composite_pipeline = Self::create_pipeline(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        );

        let bloom_texture = Self::create_bloom_texture(device, width, height, input_texture_format);

        Self {
            composite_pipeline,
            bloom_texture,
            brightness_extraction_pipeline,
            kawase_blur_pipeline,
            input_texture_format,
        }
    }

    /// Use this instead of `Bloom::execute()`
    pub fn bloom(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input_texture: &wgpu::Texture,
        output_texture: &wgpu::Texture,
    ) {
        // write brightness to the output because why not
        self.brightness_extraction_pipeline
            .execute(device, encoder, input_texture, output_texture);
        // use that output to put the blurred bloom on the bloom texture
        self.kawase_blur_pipeline
            .execute(device, encoder, output_texture, &self.bloom_texture);
        // composite input texture with the hardcoded bloom to the output texture
        self.execute(device, encoder, input_texture, output_texture);
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.bloom_texture =
            Self::create_bloom_texture(device, width, height, self.input_texture_format);
    }
}
