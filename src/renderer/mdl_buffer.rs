use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::bsp_loader::MdlEntityInfo;

use super::{
    camera::Camera,
    texture_buffer::texture_array::{TextureArrayBuffer, create_texture_array},
    utils::eightbpp_to_rgba8,
};

pub struct MdlBuffer {
    pub vertices: Vec<MdlVertexBuffer>,
    pub textures: Vec<TextureArrayBuffer>,
    pub mvps: MdlMvp,
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct MdlVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    layer: u32,     // texture idx from texture array
    model_idx: u32, // index of the model because a buffer might contain lots of models
}

impl MdlVertex {
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // uv
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                // layer
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 20,
                    shader_location: 2,
                },
                // model idx
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 24,
                    shader_location: 3,
                },
            ],
        }
    }
}

pub struct MdlVertexBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub texture_array_idx: usize,
}

pub struct MdlMvp {
    pub bind_group: wgpu::BindGroup,
    pub buffer: wgpu::Buffer,
    pub entity_infos: Vec<MdlEntityInfo>,
}

impl MdlMvp {
    fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("model view projection bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        }
    }

    fn create_mvp(device: &wgpu::Device, entity_infos: &[&MdlEntityInfo]) -> Self {
        let matrices: Vec<[[f32; 4]; 4]> = entity_infos
            .iter()
            .map(|info| info.model_view_projection.into())
            .collect();

        let mvp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection array buffer"),
            contents: bytemuck::cast_slice(&matrices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let mvp_bind_group_layout =
            device.create_bind_group_layout(&MdlMvp::bind_group_layout_descriptor());

        let mvp_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view projection array bind group"),
            layout: &mvp_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mvp_buffer.as_entire_binding(),
            }],
        });

        MdlMvp {
            bind_group: mvp_bind_group,
            buffer: mvp_buffer,
            entity_infos: entity_infos
                .iter()
                .map(|s| {
                    // the fuck?
                    let what = s.to_owned().to_owned();
                    what
                })
                .collect(),
        }
    }
}

pub struct MdlLoader;

impl MdlLoader {
    pub fn create_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
    ) -> wgpu::RenderPipeline {
        let mdl_shader = device.create_shader_module(wgpu::include_wgsl!("./shader/mdl.wgsl"));

        let texture_array_bind_group_layout =
            device.create_bind_group_layout(&TextureArrayBuffer::bind_group_layout_descriptor());

        let camera_bind_group_layout =
            device.create_bind_group_layout(&Camera::bind_group_layout_descriptor());

        let mvp_bind_group_layout =
            device.create_bind_group_layout(&MdlMvp::bind_group_layout_descriptor());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &texture_array_bind_group_layout,
                &mvp_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let mdl_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mdl render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mdl_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[MdlVertex::buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mdl_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &fragment_targets
                    .into_iter()
                    .map(|v| Some(v))
                    .collect::<Vec<Option<wgpu::ColorTargetState>>>(),
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        mdl_render_pipeline
    }

    // load multiple models into 1 vertex buffer
    // can be used for loading 1 model
    pub fn load_mdls(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mdls: &[&mdl::Mdl],
        mdl_entity_infos: &[&MdlEntityInfo],
    ) -> MdlBuffer {
        // textures
        // load texures first so we can do everything in 1 bind and 1 draw call
        let (lookup_table, texture_arrays) = Self::load_textures(device, queue, mdls);

        // process the model faces
        // create a new pipe line because mdl uses triangle strip
        // (texture array idx, interleaved vertex data)
        // no need for indices data because ~~we are rendering strip~~
        // we are rendering all triangles
        let mut batches = HashMap::<usize, (Vec<MdlVertex>, Vec<u32>)>::new();

        mdls.iter().enumerate().for_each(|(mdl_index, mdl)| {
            mdl.bodyparts.iter().for_each(|bodypart| {
                bodypart.models.iter().for_each(|model| {
                    model.meshes.iter().for_each(|mesh| {
                        // one mesh has the same texture everything
                        let texture_idx = mesh.header.skin_ref as usize;
                        let texture = &mdl.textures[texture_idx];
                        let (width, height) = texture.dimensions();

                        // let triangle_list = triangle_strip_to_triangle_list(&mesh.vertices);

                        mesh.triangles.iter().for_each(|triangles| {
                            // it is possible for a mesh to have both fan and strip run
                            let (is_strip, triverts) = match triangles {
                                mdl::MeshTriangles::Strip(triverts) => (true, triverts),
                                mdl::MeshTriangles::Fan(triverts) => (false, triverts),
                            };

                            // now just convert triverts into mdl vertex data
                            // then do some clever stuff with index buffer to make it triangle list

                            let (array_idx, layer_idx) = lookup_table[mdl_index][texture_idx];
                            let batch = batches.entry(array_idx).or_insert((vec![], vec![]));

                            let new_vertices_offset = batch.0.len();

                            // create vertex buffer here
                            let vertices = triverts.iter().map(|trivert| {
                                let [u, v] = [
                                    trivert.header.s as f32 / width as f32,
                                    trivert.header.t as f32 / height as f32,
                                ];

                                MdlVertex {
                                    pos: trivert.vertex.to_array(),
                                    uv: [u, v],
                                    layer: layer_idx as u32,
                                    model_idx: mdl_index as u32,
                                }
                            });

                            batch.0.extend(vertices);

                            let mut index_buffer: Vec<u32> = vec![];

                            // create index buffer here
                            // here we will create triangle list
                            // deepseek v3 zero shot this one
                            if is_strip {
                                for i in 0..triverts.len().saturating_sub(2) {
                                    if i % 2 == 0 {
                                        // Even-indexed triangles
                                        index_buffer.push(new_vertices_offset as u32 + i as u32);
                                        index_buffer
                                            .push(new_vertices_offset as u32 + (i + 1) as u32);
                                        index_buffer
                                            .push(new_vertices_offset as u32 + (i + 2) as u32);
                                    } else {
                                        // Odd-indexed triangles (flip winding order)
                                        index_buffer
                                            .push(new_vertices_offset as u32 + (i + 1) as u32);
                                        index_buffer.push(new_vertices_offset as u32 + i as u32);
                                        index_buffer
                                            .push(new_vertices_offset as u32 + (i + 2) as u32);
                                    }
                                }
                            } else {
                                let first_index = new_vertices_offset as u32;
                                for i in 1..triverts.len().saturating_sub(1) {
                                    index_buffer.push(first_index);
                                    index_buffer.push(new_vertices_offset as u32 + i as u32);
                                    index_buffer.push(new_vertices_offset as u32 + (i + 1) as u32);
                                }
                            }

                            batch.1.extend(index_buffer);
                        });
                    });
                });
            });
        });

        let vertex_buffers: Vec<MdlVertexBuffer> = batches
            .into_iter()
            .map(|(texture_array_idx, (vertices, indices))| {
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mdl vertex buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mdl index buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });

                MdlVertexBuffer {
                    vertex_buffer,
                    index_buffer,
                    index_count: vertices.len() as u32,
                    texture_array_idx,
                }
            })
            .collect();

        MdlBuffer {
            vertices: vertex_buffers,
            textures: texture_arrays,
            mvps: MdlMvp::create_mvp(device, mdl_entity_infos),
        }
    }

    fn load_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mdls: &[&mdl::Mdl],
    ) -> (Vec<Vec<(usize, usize)>>, Vec<TextureArrayBuffer>) {
        let model_textures: Vec<Vec<_>> = mdls
            .iter()
            .map(|mdl| {
                mdl.textures
                    .iter()
                    .map(|texture| {
                        eightbpp_to_rgba8(
                            &texture.image,
                            &texture.palette,
                            texture.dimensions().0,
                            texture.dimensions().1,
                        )
                    })
                    .collect()
            })
            .collect();

        // this bucket groups all textures with the same dimension together
        // key is dimension
        // value is Vec<(model index, texture indices)>
        // it is so that we can look up the model from which bucket
        let mut buckets: HashMap<(u32, u32), Vec<(usize, usize)>> = HashMap::new();

        // here we build the first look up table
        // we still don't know what the bucket idx is but we do know bucket texture idx
        // to fix that, we just have to iterate through the hashmap and do some comparisons (hopefully cheap)
        model_textures
            .iter()
            .enumerate()
            .for_each(|(model_idx, model)| {
                model.iter().enumerate().for_each(|(texture_idx, texture)| {
                    buckets
                        .entry(texture.dimensions())
                        .or_insert(vec![])
                        .push((model_idx, texture_idx));
                });
            });

        // second look up table is a vector of vectors
        // first index is the model index
        // second index is the texture index
        // the result is the texture array buffer and the index of the texture
        let mut lookup_table: Vec<Vec<(usize, usize)>> = mdls
            .iter()
            .map(|mdl| vec![(0, 0); mdl.textures.len()])
            .collect();

        let texture_arrays: Vec<TextureArrayBuffer> = buckets
            .iter()
            .enumerate()
            .map(|(bucket_idx, (_, texture_indices))| {
                // add the texture indices into our simpler lookup table
                texture_indices.iter().enumerate().for_each(
                    |(layer_idx, &(model_idx, texture_idx))| {
                        lookup_table[model_idx][texture_idx] = (bucket_idx, layer_idx);
                    },
                );

                let ref_vec = texture_indices
                    .iter()
                    .map(|&(model_idx, texture_idx)| &model_textures[model_idx][texture_idx])
                    .collect::<Vec<_>>();

                create_texture_array(device, queue, &ref_vec).unwrap()
            })
            .collect();

        (lookup_table, texture_arrays)
    }
}
