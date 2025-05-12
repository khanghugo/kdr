use std::{array::from_fn, f32::consts::TAU};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::pp_trait::{PostProcessingModule, PostProcessingPipeline};

const COLOR_COUNT: usize = 12;

fn generate_oklab_color(count: usize) -> Vec<[f32; 4]> {
    let chroma_range = 0.1..=0.4;
    let lightness_range = 0.05..=0.5;
    let hue_start: f32 = rand::random_range(0.0..TAU);

    let lightness: f32 = rand::random_range(lightness_range);

    (0..count)
        .map(|curr_color| {
            let hue = hue_start + (curr_color as f32 * TAU * 0.618034).rem_euclid(TAU);
            let chroma: f32 = rand::random_range(chroma_range.clone());

            let a = chroma * hue.cos();
            let b = chroma * hue.sin();

            let oklab_color = oklab::Oklab { l: lightness, a, b };
            let srgb = oklab_color.to_srgb_f32();

            [srgb.r, srgb.g, srgb.b, 1.0]
        })
        .collect()
}

pub struct Posterize {
    pipeline: PostProcessingPipeline,
    color_buffer: wgpu::Buffer,
}

const MAX_COLOR: usize = 64;
#[derive(Pod, Zeroable, Clone, Copy)]
#[repr(C)]
struct ColorBuffer {
    color_count: u32,
    _padding: [u8; 12],
    colors: [[f32; 4]; MAX_COLOR],
}

impl PostProcessingModule for Posterize {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::include_wgsl!("./posterize.wgsl"))
    }

    fn post_processing_effect_name() -> &'static str {
        "posterize"
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
            entries: &Self::bind_group_layout_descriptor_entries(),
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
                    resource: wgpu::BindingResource::Sampler(self.get_sampler()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.color_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        })
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

        let color_buffer = Self::color_buffer(device);

        Self {
            pipeline,
            color_buffer,
        }
    }
}

impl Posterize {
    fn bind_group_layout_descriptor_entries() -> &'static [wgpu::BindGroupLayoutEntry] {
        vec![
            // view
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
            // sampler
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // color count and colors
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ]
        .leak()
    }

    fn color_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        let color_count = COLOR_COUNT;
        let mut colors = generate_oklab_color(color_count);

        colors.resize(MAX_COLOR, [0f32; 4]);

        let color_buffer = ColorBuffer {
            color_count: color_count as u32,
            _padding: [0; 12],
            colors: from_fn(|i| colors.get(i).unwrap_or(&[0f32; 4]).to_owned()),
        };

        let binding = [color_buffer];
        let cast_bytes: &[u8] = bytemuck::cast_slice(&binding);

        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection entity buffer"),
            contents: cast_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        color_buffer
    }

    pub fn update_color_buffer(&mut self) {
        let new_colors = generate_oklab_color(COLOR_COUNT);
        let new_color_casted: &[u8] = bytemuck::cast_slice(&new_colors);

        self.pipeline
            .queue
            .write_buffer(&self.color_buffer, 16, new_color_casted);
    }
}
