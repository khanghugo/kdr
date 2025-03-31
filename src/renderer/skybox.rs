use image::RgbaImage;
use wgpu::util::DeviceExt;

use crate::renderer::camera::CameraBuffer;

pub struct SkyboxBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub bind_group: wgpu::BindGroup,
}

impl Drop for SkyboxBuffer {
    fn drop(&mut self) {
        self.texture.destroy();
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}

impl SkyboxBuffer {
    fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("skybox bind group layout descriptor"),
            entries: &[
                // cube texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
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

    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: 3 * 4, // 3 floats, we only need the vertex from the pre-made vertices
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        }
    }

    fn vertices() -> [[f32; 3]; 24] {
        // i dont even know what order they are in
        // can change the texture instead because that makes more sense
        [
            [1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, -1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ]
    }

    fn indices() -> [u32; 36] {
        [
            0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16,
            17, 18, 18, 19, 16, 20, 21, 22, 22, 23, 20,
        ]
    }
}

pub struct SkyboxLoader {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl SkyboxLoader {
    pub fn create_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_texture_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("./shader/skybox.wgsl"));

        let skybox_bind_group_layout =
            device.create_bind_group_layout(&SkyboxBuffer::bind_group_layout_descriptor());

        let camera_bind_group_layout =
            device.create_bind_group_layout(&CameraBuffer::bind_group_layout_descriptor());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skybox pipeline layout"),
            bind_group_layouts: &[
                // the camera view and projection for vertex shader
                &camera_bind_group_layout,
                // and then normal bind group layout for fragment shader
                &skybox_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("skybox render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[SkyboxBuffer::vertex_buffer_layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Front), // the triangles facing outward
                // cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            // basically no depth but we need it to be compatible with opaque pass
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_texture_format,
                depth_write_enabled: false,
                // always draw on top
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        pass_op: wgpu::StencilOperation::Keep,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0x00,
                    ..Default::default()
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &fragment_targets
                    .into_iter()
                    .map(|v| Some(v))
                    .collect::<Vec<Option<wgpu::ColorTargetState>>>(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout: skybox_bind_group_layout,
        }
    }

    pub fn load_skybox(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &Vec<RgbaImage>,
    ) -> SkyboxBuffer {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skybox texture"),
            size: wgpu::Extent3d {
                width: images[0].width(),
                height: images[0].height(),
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // if the orientation is wrong, check with the input images
        // try until it works
        for (i, image) in images.iter().enumerate() {
            if i >= 6 {
                break;
            }

            let (width, height) = image.dimensions();

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                image,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width), // rgba
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("skybox texture view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skybox sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let vertices = SkyboxBuffer::vertices();

        let indices = SkyboxBuffer::indices();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skybox vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skybox index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let index_count = indices.len() as u32;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skybox bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        SkyboxBuffer {
            texture,
            view,
            sampler,
            vertex_buffer,
            index_buffer,
            index_count,
            bind_group,
        }
    }
}
