use crate::renderer::utils::FullScrenTriVertexShader;

pub struct PostProcessingPipeline {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,
    pub sampler: wgpu::Sampler,
}

pub trait PostProcessingModule {
    fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule;

    fn post_processing_effect_name() -> &'static str;

    fn fragment_shader_entry() -> &'static str {
        "fs_main"
    }

    fn primitive_state() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        }
    }

    fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Self::sampler_label(),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        })
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&Self::bind_group_layout_descriptor())
    }

    fn bind_group_layout_descriptor_label() -> Option<&'static str> {
        Some(
            format!(
                "{} bind group layout descriptor",
                Self::post_processing_effect_name()
            )
            .leak(),
        )
    }

    fn _bind_group_layout_label() -> Option<&'static str> {
        Some(format!("{} bind group layout", Self::post_processing_effect_name()).leak())
    }

    fn bind_group_label() -> Option<&'static str> {
        Some(format!("{} bind group", Self::post_processing_effect_name()).leak())
    }

    fn pipeline_layout_label() -> Option<&'static str> {
        Some(format!("{} pipeline layout", Self::post_processing_effect_name()).leak())
    }

    fn pipeline_label() -> Option<&'static str> {
        Some(format!("{} pipeline", Self::post_processing_effect_name()).leak())
    }

    fn sampler_label() -> Option<&'static str> {
        Some(format!("{} sampler", Self::post_processing_effect_name()).leak())
    }

    // not sure if this should be used becuase it might leak multiple time
    fn _render_pass_label() -> Option<&'static str> {
        Some(format!("{} render pass", Self::post_processing_effect_name()).leak())
    }

    fn get_bind_group_layout(&self) -> &wgpu::BindGroupLayout;
    fn get_pipeline(&self) -> &wgpu::RenderPipeline;
    fn get_sampler(&self) -> &wgpu::Sampler;

    // Modify this if different bind group
    fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Self::bind_group_layout_descriptor_label(),
            entries: &[
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
            ],
        }
    }

    // Modify this if different bind group
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
            ],
        })
    }

    fn create_pipeline(
        device: &wgpu::Device,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
    ) -> PostProcessingPipeline {
        let shader = Self::create_shader_module(device);
        let sampler = Self::create_sampler(device);
        let bind_group_layout = Self::create_bind_group_layout(device);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Self::pipeline_layout_label(),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Self::pipeline_label(),
            layout: Some(&pipeline_layout),
            vertex: fullscreen_tri_vertex_shader.vertex_state(),
            primitive: Self::primitive_state(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(Self::fragment_shader_entry()),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: input_texture_format,
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
            }),
            multiview: None,
            cache: None,
        });

        PostProcessingPipeline {
            bind_group_layout,
            pipeline,
            sampler,
        }
    }

    fn execute(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input_texture: &wgpu::Texture,
        output_texture: &wgpu::Texture,
    ) {
        let input_texture_view = input_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let output_texture_view =
            output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&format!(
                "{} render pass",
                Self::post_processing_effect_name()
            )),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output_texture_view,
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

        let bind_group = self.create_bind_group(device, &input_texture_view);

        rpass.set_pipeline(self.get_pipeline());
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    fn new(
        device: &wgpu::Device,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
    ) -> Self;
}
