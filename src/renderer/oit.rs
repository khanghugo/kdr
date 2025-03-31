use super::utils::FullScrenTriVertexShader;

type WBOITRenderTargetColor = [wgpu::ColorTargetState; 2];

#[derive(Clone)]
pub struct WBOITRenderTarget();

impl WBOITRenderTarget {
    pub fn targets() -> WBOITRenderTargetColor {
        [
            // acc
            wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            },
            // reveal
            wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R16Float,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::OneMinusSrc,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            },
        ]
    }
    // i kind of giving up on refactoring this...
}

// change stuff here to switch OIT methods
pub type OITRenderTargetColor = WBOITRenderTargetColor;
pub type OITRenderTarget = WBOITRenderTarget;

// vibe coding the whole thing
pub struct OITResolver {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    accum_texture: wgpu::Texture,
    reveal_texture: wgpu::Texture,
    accum_view: wgpu::TextureView,
    reveal_view: wgpu::TextureView,
}

impl Drop for OITResolver {
    fn drop(&mut self) {
        self.accum_texture.destroy();
        self.reveal_texture.destroy();
    }
}

impl OITResolver {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        input_opaque_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
    ) -> Self {
        // Create shader module
        let resolve_shader =
            device.create_shader_module(wgpu::include_wgsl!("./shader/resolve.wgsl"));

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OIT Resolve Bind Group Layout"),
            entries: &[
                // acc tex
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
                // reveal tex
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
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("OIT Resolve Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // render targets
        let [accum_texture, reveal_texture] = Self::create_render_targets(&device, width, height);
        let accum_view = accum_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let reveal_view = reveal_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OIT Resolve Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&accum_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&reveal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("OIT Resolve Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("OIT Resolve Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: fullscreen_tri_vertex_shader.vertex_state(),
            fragment: Some(wgpu::FragmentState {
                module: &resolve_shader,
                entry_point: Some("resolve_fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: input_opaque_texture_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Important for fullscreen pass!
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            accum_texture,
            reveal_texture,
            accum_view,
            reveal_view,
        }
    }

    fn create_render_targets(device: &wgpu::Device, width: u32, height: u32) -> [wgpu::Texture; 2] {
        let accum_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("OIT Accum"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let reveal_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("OIT Reveal"),
            size: accum_texture.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        [accum_texture, reveal_texture]
    }

    pub fn composite(&self, rpass: &mut wgpu::RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1); // Draw fullscreen triangle
    }

    pub fn render_pass_color_attachments(&self) -> [Option<wgpu::RenderPassColorAttachment>; 2] {
        [
            Some(wgpu::RenderPassColorAttachment {
                view: &self.accum_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &self.reveal_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            }),
        ]
    }
}
