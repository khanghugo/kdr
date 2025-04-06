// deepseek wrote the whole thing wtf

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub fn calculate_mipmap_count(width: u32, height: u32) -> u32 {
    (width.max(height) as f32).log2().floor() as u32 + 1
}

#[derive(Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct LayerUniform {
    layer: u32,
    _padding: [u32; 3],
}

/// Only works on texture array
pub struct MipMapGenerator<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl<'a> MipMapGenerator<'a> {
    /// Only works on texture array
    pub fn create_render_pipeline(
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        texture_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader/mipmap.wgsl"));

        let bind_group_layout_descriptor = wgpu::BindGroupLayoutDescriptor {
            label: Some("mipmap bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        // cannot do 4 minimum bytes
                        // """
                        // In the provided shader, the type given for group 0 binding 2 has a size of 4.
                        // As the device does not support `DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`,
                        // the type must have a size that is a multiple of 16 bytes.
                        // """
                        // And when I tried 16 bytes.
                        // """
                        // Buffer structure size 32, added to one element of an unbound array, if it's the last field,
                        // ended up greater than the given `min_binding_size`, which is 16
                        // """
                        // Whatever.
                        // TODO give a shit
                        min_binding_size: NonZeroU64::new(32),
                    },
                    count: None,
                },
            ],
        };

        let bind_group_layout = device.create_bind_group_layout(&bind_group_layout_descriptor);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mipmap pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mipmap generator"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(texture_format.clone().into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
        }
    }

    pub fn generate_mipmaps_texture_array(
        &self,
        texture: &wgpu::Texture,
        mip_level_count: u32,
        array_layer_count: u32,
    ) {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mipmap generator sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mipmap generation encoder"),
            });

        for layer in 0..array_layer_count {
            for mip_level in 1..mip_level_count {
                let src_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("mipmap source view"),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: mip_level - 1,
                    mip_level_count: Some(1),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    usage: None,
                });

                let dst_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("mipmap destination view"),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    usage: None,
                });

                let layer_uniform = LayerUniform {
                    layer,
                    _padding: [0u32; 3],
                };

                let layer_uniform_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("layer uniform buffer"),
                            contents: bytemuck::cast_slice(&[layer_uniform, layer_uniform]),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        });

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("mipmap bind group"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&src_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &layer_uniform_buffer,
                                offset: 0,
                                size: None,
                            }),
                        },
                    ],
                });

                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mipmap render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, &bind_group, &[]);
                rpass.draw(0..3, 0..1); // Draw a full-screen triangle
            }
        }

        self.queue.submit(Some(encoder.finish()));
    }
}
