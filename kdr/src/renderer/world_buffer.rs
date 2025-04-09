use std::collections::HashMap;

use bytemuck_derive::{Pod, Zeroable};
use image::RgbaImage;
use tracing::{info, warn};
use wgpu::util::DeviceExt;

use loader::bsp_resource::{BspResource, CustomRender, EntityModel, WorldEntity};

use super::{
    bsp_lightmap::LightMapAtlasBuffer,
    camera::CameraBuffer,
    mvp_buffer::MvpBuffer,
    texture_buffer::texture_array::{TextureArrayBuffer, create_texture_array},
    utils::{face_vertices, get_bsp_textures, get_mdl_textures, vertex_uv},
};

/// Key: (World Entity Index, Texture Index)
///
/// Value: (Texture Array Index, Texture Index)
type WorldTextureLookupTable = HashMap<(usize, usize), (usize, usize)>;

/// Key: Batch Index aka Texture Array Index
///
/// Value: (World Vertex Array, Index Array)
type BatchLookup = HashMap<usize, (Vec<WorldVertex>, Vec<u32>)>;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
/// Common vertex data structure for both bsp and mdl
pub struct WorldVertex {
    pos: [f32; 3],
    tex_coord: [f32; 2],
    normal: [f32; 3],
    layer: u32,
    model_idx: u32,
    // type of the vertex, bsp vertex or mdl vertex
    // 0: bsp, 1: mdl
    type_: u32,
    // for bsp: [lightmap_u, lightmap_v, renderamt]
    // for mdl: unused
    data_a: [f32; 3],
    // for bsp: [rendermode, is_sky]
    // for mdl: [textureflag, unused]
    data_b: [u32; 2],
}

impl WorldVertex {
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
                // texcoord
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                // normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 20,
                    shader_location: 2,
                },
                // texture layer
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 32,
                    shader_location: 3,
                },
                // model index to get model view projection matrix
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 36,
                    shader_location: 4,
                },
                // vertex type
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 40,
                    shader_location: 5,
                },
                // data_a
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 44,
                    shader_location: 6,
                },
                // data_b
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32x2,
                    offset: 56,
                    shader_location: 7,
                },
            ],
        }
    }
}

pub struct WorldVertexBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: usize,
    pub texture_array_index: usize,
}

impl Drop for WorldVertexBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}

pub struct WorldBuffer {
    pub opaque: Vec<WorldVertexBuffer>,
    // only 1 buffer because OIT
    pub transparent: Vec<WorldVertexBuffer>,
    pub textures: Vec<TextureArrayBuffer>,
    pub bsp_lightmap: LightMapAtlasBuffer,
    pub mvp_buffer: MvpBuffer,
    // seems dumb, but it works. The only downside is that it feeds in a maybe big vertex buffer containing a lot of other vertices
    // but the fact that we can filter it inside the shader is nice enough
    // it works and it looks dumb so that is why i have to write a lot here
    // a map might not have sky texture so this is optional
    // the index is for opaque buffer vector
    pub skybrush_batch_index: Option<usize>,
}

pub enum WorldPipelineType {
    /// Standard Z Pre Pass
    ZPrepass,
    /// Masking SKY texture in the scene with depth value of 1.0 (farthest possible)
    ///
    /// With this, it is easier to draw skybox over it while also being able to occlude objects behind it.
    ///
    /// This means 3D skybox tricks don't work :()
    SkyboxMask,
    Opaque,
    Transparent,
}

pub struct WorldLoader;

impl WorldLoader {
    pub fn create_opaque_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::Opaque,
        )
    }

    pub fn create_transparent_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::Transparent,
        )
    }

    pub fn create_z_prepass_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::ZPrepass,
        )
    }

    pub fn create_skybox_mask_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::SkyboxMask,
        )
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
        pipeline_type: WorldPipelineType,
    ) -> wgpu::RenderPipeline {
        let world_shader = device.create_shader_module(wgpu::include_wgsl!("./shader/world.wgsl"));

        // common data
        let texture_array_bind_group_layout =
            device.create_bind_group_layout(&TextureArrayBuffer::bind_group_layout_descriptor());

        let camera_bind_group_layout =
            device.create_bind_group_layout(&CameraBuffer::bind_group_layout_descriptor());

        let mvp_bind_group_layout =
            device.create_bind_group_layout(&MvpBuffer::bind_group_layout_descriptor());

        // bsp specific
        let lightmap_bind_group_layout =
            device.create_bind_group_layout(&LightMapAtlasBuffer::bind_group_layout_descriptor());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &camera_bind_group_layout,        // 0
                &mvp_bind_group_layout,           // 1
                &texture_array_bind_group_layout, // 2
                &lightmap_bind_group_layout,      // 3
            ],
            push_constant_ranges: &[],
        });

        let fragment_targets = fragment_targets
            .into_iter()
            .map(|v| Some(v))
            .collect::<Vec<Option<wgpu::ColorTargetState>>>();

        // dont write any more depth after z prepass
        let depth_write_enabled = match pipeline_type {
            WorldPipelineType::ZPrepass => true,
            // if i somehow start doign z prepass again, opaque should not write to depth
            WorldPipelineType::Opaque => true,
            WorldPipelineType::Transparent | WorldPipelineType::SkyboxMask => false,
        };

        let pipeline_label = match pipeline_type {
            WorldPipelineType::ZPrepass => "world z prepass render pipeline",
            WorldPipelineType::Opaque => "world opaque render pipeline",
            WorldPipelineType::Transparent => "world transparent render pipeline",
            WorldPipelineType::SkyboxMask => "world skybox mask render pipeline",
        };

        let depth_compare = match pipeline_type {
            WorldPipelineType::ZPrepass => wgpu::CompareFunction::Less,
            WorldPipelineType::Opaque => wgpu::CompareFunction::LessEqual,
            WorldPipelineType::Transparent => wgpu::CompareFunction::Less,
            // need to write stencil in a way that the skybrushes are behind some objects
            WorldPipelineType::SkyboxMask => wgpu::CompareFunction::LessEqual,
        };

        let stencil_state: wgpu::StencilState = match pipeline_type {
            WorldPipelineType::SkyboxMask => wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    pass_op: wgpu::StencilOperation::Replace,
                    fail_op: wgpu::StencilOperation::Keep,
                    ..Default::default()
                },
                read_mask: 0xFF,
                write_mask: 0xFF,
                ..Default::default()
            },
            WorldPipelineType::ZPrepass
            | WorldPipelineType::Opaque
            | WorldPipelineType::Transparent => Default::default(),
        };

        let vertex_shader_entry_point = match pipeline_type {
            WorldPipelineType::SkyboxMask => "skybox_mask_vs",
            // does nothing
            WorldPipelineType::ZPrepass => "skybox_mask_vs",
            WorldPipelineType::Opaque | WorldPipelineType::Transparent => "vs_main",
        };

        let world_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(pipeline_label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &world_shader,
                    entry_point: Some(vertex_shader_entry_point),
                    compilation_options: Default::default(),
                    buffers: &[WorldVertex::buffer_layout()],
                },
                fragment: match pipeline_type {
                    WorldPipelineType::ZPrepass | WorldPipelineType::SkyboxMask => None,
                    WorldPipelineType::Opaque | WorldPipelineType::Transparent => {
                        Some(wgpu::FragmentState {
                            module: &world_shader,
                            entry_point: Some(
                                if matches!(pipeline_type, WorldPipelineType::Opaque) {
                                    "fs_opaque"
                                } else {
                                    "fs_transparent"
                                },
                            ),
                            compilation_options: Default::default(),
                            targets: &fragment_targets,
                        })
                    }
                },
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: Some(wgpu::Face::Back),
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled,
                    depth_compare,
                    stencil: stencil_state,
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        world_render_pipeline
    }

    pub fn load_world(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource: &BspResource,
    ) -> WorldBuffer {
        let lightmap = LightMapAtlasBuffer::load_lightmap(device, queue, &resource.bsp);
        let (lookup_table, texture_arrays) = Self::load_textures(device, queue, resource);
        let (opaque_batch, transparent_batch) =
            create_batch_lookups(resource, &lookup_table, &lightmap);

        let opaque_vertex_buffer = create_world_vertex_buffer(device, opaque_batch);
        let transparent_vertex_buffer = create_world_vertex_buffer(device, transparent_batch);

        // turn to vec and then sort by key
        let mut entity_infos: Vec<&WorldEntity> =
            resource.entity_dictionary.iter().map(|(_, v)| v).collect();

        entity_infos.sort_by_key(|v| v.world_index);

        let entity_infos: Vec<&WorldEntity> = entity_infos.into_iter().map(|v| v).collect();

        let mvp_buffer = MvpBuffer::create_mvp(device, &entity_infos);

        // need to find which buffer sky brushes are in
        let skybrush_batch_index = resource
            .bsp
            .textures
            .iter()
            .enumerate()
            .find(|(_, texture)| texture.texture_name.get_string_standard() == "SKY")
            .and_then(|(idx, _)| lookup_table.get(&(0, idx)))
            .map(|&(tex_array_idx, _)| tex_array_idx)
            .and_then(|skybrush_texture_array_index| {
                opaque_vertex_buffer
                    .iter()
                    .enumerate()
                    .find(|(_, e)| e.texture_array_index == skybrush_texture_array_index)
            })
            .map(|(idx, _)| idx);

        WorldBuffer {
            opaque: opaque_vertex_buffer,
            transparent: transparent_vertex_buffer,
            textures: texture_arrays,
            bsp_lightmap: lightmap,
            mvp_buffer,
            skybrush_batch_index,
        }
    }

    fn load_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource: &BspResource,
    ) -> (WorldTextureLookupTable, Vec<TextureArrayBuffer>) {
        // key is the entity name
        // value is the texture array inside that model associated with that world entity
        // So, if we have multiple models, they all will have the same key and the same textures
        // In the case of worldspawn, just call it "worldspawn".
        let mut entity_textures: HashMap<String, Vec<RgbaImage>> = HashMap::new();

        // insert textures
        resource
            .entity_dictionary
            .iter()
            .for_each(|(_, entity)| match entity.model {
                EntityModel::Bsp => {
                    // hardcoded for all bsp brushes to use textures from worldspawn
                    entity_textures.insert(
                        "worldspawn".to_string(),
                        get_bsp_textures(&resource.bsp, &resource.external_wad_textures),
                    );
                }
                EntityModel::Mdl(ref mdl_name) => {
                    if entity_textures.contains_key(mdl_name) {
                        return;
                    }

                    let Some(mdl_data) = resource.model_lookup.get(mdl_name) else {
                        return;
                    };

                    entity_textures.insert(mdl_name.to_string(), get_mdl_textures(&mdl_data));
                }
                EntityModel::Sprite => {
                    todo!("cannot load sprite at the moment")
                }
                // for all other bsp brushes, they don't need to load their own textures
                _ => (),
            });

        // looking up which texture array to use from dimensions
        // key is dimensions
        // value is Vec<(model name, texture indices)>
        // So, we have to somehow later translate that model name to world entity index
        let mut texture_arrays_look_up: HashMap<(u32, u32), Vec<(String, usize)>> = HashMap::new();

        // We only care about models that have textures
        let entities_with_textures_names = resource
            .entity_dictionary
            .iter()
            .filter_map(|(_, entity)| match entity.model {
                EntityModel::Bsp => Some(("worldspawn", entity.world_index)),
                EntityModel::Mdl(ref name) => Some((name, entity.world_index)),
                EntityModel::Sprite => todo!("not implemented for sprite loading"),
                _ => None,
            })
            .collect::<Vec<_>>();

        // iterate over entities with textures, again, we want to make sure that the value is (world entity index, _)
        entities_with_textures_names
            .iter()
            .for_each(|(model_name, _)| {
                let Some(textures) = entity_textures.get(*model_name) else {
                    return;
                };

                textures
                    .iter()
                    .enumerate()
                    .for_each(|(texture_idx, texture)| {
                        texture_arrays_look_up
                            .entry(texture.dimensions())
                            .or_insert(vec![])
                            .push((model_name.to_string(), texture_idx));
                    });
            });

        // result look up table
        // look up the texture array buffer and the texture index from the entity index and its texture
        // key is (world entity index, texture index)
        // value is (texture array buffer index, index of the texture in the texture array buffer)
        let mut lookup_table: WorldTextureLookupTable = HashMap::new();

        // Now, our look up table will have more entries than our texture arrays.
        // So, make that work somehow.
        // I won't even begin to explain the fuckery this is. No LLM usage by the way.
        let texture_arrays: Vec<TextureArrayBuffer> = texture_arrays_look_up
            .iter()
            .enumerate()
            .map(|(bucket_idx, (_, texture_indices))| {
                // add the texture indices into our lookup table
                texture_indices.iter().enumerate().for_each(
                    |(layer_idx, (model_name, texture_idx))| {
                        // in this, we already have our vector of entites with model name
                        // just iterate over it to find them and add them here
                        entities_with_textures_names
                            .iter()
                            .filter(|(curr_model_name, _)| *curr_model_name == model_name)
                            .for_each(|(_, entity_world_index)| {
                                lookup_table.insert(
                                    (*entity_world_index, *texture_idx),
                                    (bucket_idx, layer_idx),
                                );
                            });
                    },
                );

                let ref_vec = texture_indices
                    .iter()
                    .map(|(world_entity_idx, texture_idx)| {
                        &entity_textures
                            .get(world_entity_idx)
                            .expect("cannot find entity")[*texture_idx]
                    })
                    .collect::<Vec<_>>();

                create_texture_array(device, queue, &ref_vec).expect("cannot make texture array")
            })
            .collect();

        let texture_count: usize = entity_textures.iter().map(|(_, v)| v.len()).sum();

        info!(
            "Created {} texture arrays of {} textures with {} entities look up",
            texture_arrays.len(),
            texture_count,
            lookup_table.len()
        );

        (lookup_table, texture_arrays)
    }
}

struct ProcessBspFaceData<'a> {
    bsp_face_index: usize,
    world_entity_index: usize,
    texture_layer_index: usize,
    face: &'a bsp::Face,
    custom_render: Option<&'a CustomRender>,
    /// 0: Normal bsp face such as opaque and transparent face
    ///
    /// 1: Sky
    ///
    /// 2: Trigger brush
    type_: u32,
}

// Returns (opaque batch lookup, transparent batch lookup)
fn create_batch_lookups(
    resource: &BspResource,
    world_texture_lookup: &WorldTextureLookupTable,
    lightmap: &LightMapAtlasBuffer,
) -> (BatchLookup, BatchLookup) {
    let mut opaque_lookup = BatchLookup::new();
    let mut transparent_lookup = BatchLookup::new();
    let bsp = &resource.bsp;

    resource.entity_dictionary.iter().for_each(|(_, entity)| {
        let world_entity_index = entity.world_index;

        let is_transparent = matches!(
            entity.model,
            EntityModel::TransparentEntityBrush(_) | EntityModel::Sprite
        );

        let assigned_lookup = if is_transparent {
            &mut transparent_lookup
        } else {
            &mut opaque_lookup
        };

        // add the world vertex based on the entity type
        match &entity.model {
            EntityModel::Bsp
            | EntityModel::OpaqueEntityBrush(_)
            | EntityModel::TransparentEntityBrush(_)
            | EntityModel::TriggerBrush(_) => {
                let bsp_model_index = match &entity.model {
                    EntityModel::Bsp => 0,
                    EntityModel::OpaqueEntityBrush((x, _)) => *x,
                    EntityModel::TransparentEntityBrush((x, _)) => *x,
                    EntityModel::TriggerBrush(x) => *x,
                    _ => unreachable!(),
                };

                let model = &bsp.models[bsp_model_index as usize];

                let first_face = model.first_face as usize;
                let face_count = model.face_count as usize;

                let faces = &bsp.faces[first_face..(first_face + face_count)];

                // trigger will have no render effect
                let is_trigger = matches!(entity.model, EntityModel::TriggerBrush(_));

                // TODO: custom render for sprite and model, just pull this out of this scope
                let custom_render = if is_transparent {
                    if let EntityModel::TransparentEntityBrush((_, custom_render)) = &entity.model {
                        Some(custom_render)
                    } else {
                        unreachable!()
                    }
                } else {
                    None
                };

                let custom_render = match &entity.model {
                    EntityModel::OpaqueEntityBrush((_, custom_render)) => Some(custom_render),
                    EntityModel::TransparentEntityBrush((_, custom_render)) => Some(custom_render),
                    _ => None,
                };

                faces
                    .iter()
                    .enumerate()
                    .for_each(|(face_index_offset, face)| {
                        let bsp_face_index = first_face + face_index_offset;

                        let texinfo = &bsp.texinfo[face.texinfo as usize];
                        let (array_idx, layer_idx) = world_texture_lookup
                            // hardcoded entity 0 because all bsp brushes use the same textures from worldspawn
                            .get(&(0, texinfo.texture_index as usize))
                            .expect("cannot get world texture");

                        let texture_name = bsp.textures[texinfo.texture_index as usize]
                            .texture_name
                            .get_string_standard();
                        let is_sky = texture_name == "SKY";

                        let face_type = if is_sky {
                            1
                        } else if is_trigger {
                            2
                        } else {
                            0
                        };

                        let face_data = ProcessBspFaceData {
                            bsp_face_index,
                            world_entity_index,
                            texture_layer_index: *layer_idx,
                            face,
                            custom_render,
                            type_: face_type,
                        };

                        let (vertices, indices) = process_bsp_face(face_data, bsp, lightmap);

                        let batch = assigned_lookup
                            .entry(*array_idx)
                            .or_insert((Vec::new(), Vec::new()));

                        // newer vertices will have their index start at 0 but we don't want that
                        // need to divide by <x> because each "vertices" has <x> floats
                        let new_vertices_offset = batch.0.len();

                        batch.0.extend(vertices);
                        batch
                            .1
                            .extend(indices.into_iter().map(|i| i + new_vertices_offset as u32));
                    });

                // create_bsp_batch_lookup(bsp)
            }
            // for some reasons this is inline but the bsp face is not
            EntityModel::Mdl(mdl_name) => {
                let Some(mdl) = resource.model_lookup.get(mdl_name) else {
                    warn!("Cannot find model `{mdl_name}` to create a batch lookup");

                    return;
                };
                mdl.bodyparts.iter().for_each(|bodypart| {
                    bodypart.models.iter().for_each(|model| {
                        model.meshes.iter().for_each(|mesh| {
                            // one mesh has the same texture everything
                            let texture_idx = mesh.header.skin_ref as usize;
                            let texture = &mdl.textures[texture_idx];
                            let texture_flags = &texture.header.flags;
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
                                let (array_idx, layer_idx) = world_texture_lookup
                                    .get(&(world_entity_index, texture_idx))
                                    .expect("cannot get world texture");
                                let batch = assigned_lookup
                                    .entry(*array_idx)
                                    .or_insert((vec![], vec![]));

                                let new_vertices_offset = batch.0.len();

                                // create vertex buffer here
                                let vertices = triverts.iter().map(|trivert| {
                                    let [u, v] = [
                                        trivert.header.s as f32 / width as f32,
                                        trivert.header.t as f32 / height as f32,
                                    ];

                                    WorldVertex {
                                        pos: trivert.vertex.to_array(),
                                        tex_coord: [u, v],
                                        normal: trivert.normal.to_array(),
                                        layer: *layer_idx as u32,
                                        // actual model index is different
                                        // because 0 is worldspawn
                                        model_idx: world_entity_index as u32,
                                        type_: 1,
                                        data_a: [0f32; 3],
                                        data_b: [texture_flags.bits() as u32; 2],
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
                                            index_buffer
                                                .push(new_vertices_offset as u32 + i as u32);
                                            index_buffer
                                                .push(new_vertices_offset as u32 + (i + 1) as u32);
                                            index_buffer
                                                .push(new_vertices_offset as u32 + (i + 2) as u32);
                                        } else {
                                            // Odd-indexed triangles (flip winding order)
                                            index_buffer
                                                .push(new_vertices_offset as u32 + (i + 1) as u32);
                                            index_buffer
                                                .push(new_vertices_offset as u32 + i as u32);
                                            index_buffer
                                                .push(new_vertices_offset as u32 + (i + 2) as u32);
                                        }
                                    }
                                } else {
                                    let first_index = new_vertices_offset as u32;
                                    for i in 1..triverts.len().saturating_sub(1) {
                                        index_buffer.push(first_index);
                                        index_buffer.push(new_vertices_offset as u32 + i as u32);
                                        index_buffer
                                            .push(new_vertices_offset as u32 + (i + 1) as u32);
                                    }
                                }

                                batch.1.extend(index_buffer);
                            });
                        });
                    });
                });
            }
            EntityModel::Sprite => todo!("sprite world vertex is not supported"),
        };
    });

    (opaque_lookup, transparent_lookup)
}

fn process_bsp_face(
    face_data: ProcessBspFaceData,
    bsp: &bsp::Bsp,
    lightmap: &LightMapAtlasBuffer,
) -> (Vec<WorldVertex>, Vec<u32>) {
    let ProcessBspFaceData {
        bsp_face_index: face_index,
        face,
        custom_render,
        world_entity_index,
        texture_layer_index,
        type_,
    } = face_data;

    let face_vertices = face_vertices(face, bsp);

    let indices = triangulate_convex_polygon(&face_vertices);

    let normal = bsp.planes[face.plane as usize].normal;
    let texinfo = &bsp.texinfo[face.texinfo as usize];

    // uv
    let miptex = &bsp.textures[texinfo.texture_index as usize];

    let vertices_texcoords: Vec<[f32; 2]> = face_vertices
        .iter()
        .map(|pos| vertex_uv(pos, &texinfo))
        .collect();

    let vertices_normalized_texcoords: Vec<[f32; 2]> = vertices_texcoords
        .iter()
        .map(|uv| [uv[0] / miptex.width as f32, uv[1] / miptex.height as f32])
        .collect();

    // lightmap
    let what = lightmap.allocations.get(&face_index);

    // https://github.com/magcius/noclip.website/blob/66595465295720f8078a53d700988241b0adc2b0/src/GoldSrc/BSPFile.ts#L285
    let (face_min_u, _face_max_u, face_min_v, _face_max_v) = get_face_uv_box(&vertices_texcoords);

    let lightmap_texcoords: Vec<[f32; 2]> = if let Some(allocation) = what {
        let lightmap_texcoords = vertices_texcoords.iter().map(|&[u, v, ..]| {
            let lightmap_u =
                ((u / 16.0) - (face_min_u / 16.0).floor() + 0.5) / allocation.lightmap_width;
            let lightmap_v =
                ((v / 16.0) - (face_min_v / 16.0).floor() + 0.5) / allocation.lightmap_height;

            [
                allocation.atlas_x + lightmap_u * allocation.atlas_width,
                allocation.atlas_y + lightmap_v * allocation.atlas_height,
            ]
        });

        lightmap_texcoords.collect()
    } else {
        vertices_normalized_texcoords
            .iter()
            .map(|_| [0., 0.])
            .collect()
    };

    let rendermode = custom_render
        .as_ref()
        .map(|v| v.rendermode)
        // amount 1 = 255 = full aka opaque
        // transparent objects have their own default so this won't be a problem
        .unwrap_or(0);

    let renderamt = custom_render
        .as_ref()
        .map(|v| v.renderamt / 255.0)
        // amount 1 = 255 = full aka opaque
        // transparent objects have their own default so this won't be a problem
        .unwrap_or(1.0);

    // collect to buffer
    let vertices: Vec<WorldVertex> = face_vertices
        .into_iter()
        .zip(vertices_normalized_texcoords.into_iter())
        .zip(lightmap_texcoords.into_iter())
        .map(|((pos, texcoord), lightmap_coord)| WorldVertex {
            pos: pos.to_array().into(),
            tex_coord: texcoord.into(),
            normal: normal.to_array().into(),
            layer: texture_layer_index as u32,
            model_idx: world_entity_index as u32,
            type_: 0,
            data_a: [lightmap_coord[0], lightmap_coord[1], renderamt],
            data_b: [rendermode as u32, type_],
        })
        .collect();

    (vertices, indices)
}

// the dimension of the face on texture coordinate
fn get_face_uv_box(uvs: &[[f32; 2]]) -> (f32, f32, f32, f32) {
    let mut min_u = uvs[0][0];
    let mut min_v = uvs[0][1];
    let mut max_u = uvs[0][0];
    let mut max_v = uvs[0][1];

    for i in 1..uvs.len() {
        let u = uvs[i][0];
        let v = uvs[i][1];

        if u < min_u {
            min_u = u;
        }
        if v < min_v {
            min_v = v;
        }
        if u > max_u {
            max_u = u;
        }
        if v > max_v {
            max_v = v;
        }
    }

    return (min_u, max_u, min_v, max_v);
}

// deepseek wrote this
// input is a winding order polygon
// the output is the vertex index
// so there is no need to do anythign to the vertex buffer, only play around with the index buffer
fn triangulate_convex_polygon(vertices: &[bsp::Vec3]) -> Vec<u32> {
    // For convex polygons, we can simply fan-triangulate from the first vertex
    // This creates triangle indices: 0-1-2, 0-2-3, 0-3-4, etc.
    assert!(vertices.len() >= 3, "Polygon needs at least 3 vertices");

    let mut indices = Vec::with_capacity((vertices.len() - 2) * 3);
    for i in 1..vertices.len() - 1 {
        indices.push(0);
        indices.push(i as u32);
        indices.push((i + 1) as u32);
    }
    indices
}

fn create_world_vertex_buffer(
    device: &wgpu::Device,
    batch_lookup: BatchLookup,
) -> Vec<WorldVertexBuffer> {
    batch_lookup
        .into_iter()
        .map(|(texture_array_index, (vertices, indices))| {
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

            WorldVertexBuffer {
                vertex_buffer,
                index_buffer,
                index_count: indices.len(),
                texture_array_index,
            }
        })
        .collect()
}
