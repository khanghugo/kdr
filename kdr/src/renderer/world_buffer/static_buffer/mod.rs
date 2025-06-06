//! Static buffer concerns about data that aren't changed in BSP.
//!
//! The BSP it self and long with models inside the map are considered static. Even when the models and entities do move around,
//! the actual data buffer don't swap out.

use std::collections::HashMap;

use common::BuildMvpResult;
use image::RgbaImage;
use loader::bsp_resource::{BspResource, CustomRender, EntityModel, ModelLookUpType, WorldEntity};
use model::create_world_model_vertices;
use tracing::{info, warn};
use world::{get_bsp_textures, process_bsp_face};

use crate::renderer::{
    bsp_lightmap::LightMapAtlasBuffer,
    mvp_buffer::MvpBuffer,
    texture_buffer::texture_array::{TextureArrayBuffer, create_texture_array},
    world_buffer::utils::{get_mdl_textures, get_sprite_textures},
};

mod model;
mod world;

use super::{
    WorldLoader, WorldVertex, WorldVertexBuffer, WorldVertexType,
    utils::{BatchLookup, create_world_vertex_buffer},
};

/// Key: (World Entity Index, Texture Index)
///
/// Value: (Texture Array Index, Texture Index)
pub(super) type WorldTextureLookupTable = HashMap<(usize, usize), (usize, usize)>;

pub(super) type MvpLookup = HashMap<usize, usize>;

pub(super) struct ProcessBspFaceData<'a> {
    pub bsp_face_index: usize,
    pub world_entity_index: usize,
    pub texture_layer_index: usize,
    pub face: &'a bsp::Face,
    pub custom_render: Option<&'a CustomRender>,
    /// 0: Normal bsp face such as opaque and transparent face
    ///
    /// 1: Sky
    ///
    /// 2: No draw brushes
    pub type_: u32,
}

pub struct WorldStaticBuffer {
    pub opaque: Vec<WorldVertexBuffer>,
    // only 1 buffer because OIT
    pub transparent: Vec<WorldVertexBuffer>,
    pub textures: Vec<TextureArrayBuffer>,
    pub bsp_lightmap: LightMapAtlasBuffer,
    pub mvp_buffer: MvpBuffer,
    /// Returns the start MVP buffer index offset of bones #1+ of skeletal models
    pub mvp_lookup: MvpLookup,
    // seems dumb, but it works. The only downside is that it feeds in a maybe big vertex buffer containing a lot of other vertices
    // but the fact that we can filter it inside the shader is nice enough
    // it works and it looks dumb so that is why i have to write a lot here
    // a map might not have sky texture so this is optional
    // the index is for opaque buffer vector
    pub skybrush_batch_index: Option<usize>,
}

impl WorldLoader {
    pub fn load_static_world(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource: &BspResource,
    ) -> WorldStaticBuffer {
        let lightmap = LightMapAtlasBuffer::load_lightmap(device, queue, &resource.bsp);

        // turn to vec and then sort by key
        let mut sorted_entity_infos: Vec<&WorldEntity> =
            resource.entity_dictionary.iter().map(|(_, v)| v).collect();

        sorted_entity_infos.sort_by_key(|v| v.world_index);

        let entity_infos: Vec<&WorldEntity> = sorted_entity_infos.into_iter().map(|v| v).collect();

        let (lookup_table, texture_arrays) =
            Self::load_static_world_textures(device, queue, resource);
        let (opaque_batch, transparent_batch, mvp_lookup) =
            create_batch_lookups(resource, &entity_infos, &lookup_table, &lightmap);

        let opaque_vertex_buffer = create_world_vertex_buffer(device, opaque_batch);
        let transparent_vertex_buffer = create_world_vertex_buffer(device, transparent_batch);

        // creating transformations
        // we have an array of 1024 mat4s
        // the index i is the transformation of entity index i
        // however, for skeletal models, the indices are appended later and they won't take the indices of actual world entities
        let mut entity_transformations = vec![];
        let mut skeletal_transformations = vec![];

        entity_infos
            .iter()
            .for_each(|entity| match entity.transformation.build_mvp(0.) {
                BuildMvpResult::Entity(matrix4) => {
                    entity_transformations.push(matrix4);
                }
                BuildMvpResult::Skeletal(matrix4s) => {
                    entity_transformations.push(matrix4s[0]);
                    skeletal_transformations.extend(&matrix4s[1..]);
                }
            });

        let transformations = [entity_transformations, skeletal_transformations].concat();
        let mvp_buffer = MvpBuffer::create_mvp(device, queue, transformations);

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

        WorldStaticBuffer {
            opaque: opaque_vertex_buffer,
            transparent: transparent_vertex_buffer,
            textures: texture_arrays,
            bsp_lightmap: lightmap,
            mvp_buffer,
            skybrush_batch_index,
            mvp_lookup,
        }
    }

    fn load_static_world_textures(
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
                EntityModel::BspMdlEntity { ref model_name, .. } => {
                    if entity_textures.contains_key(model_name) {
                        return;
                    }

                    let Some(mdl_data) = resource.model_lookup.get(model_name) else {
                        warn!("cannot get model for loading texture {}", model_name);
                        return;
                    };

                    let ModelLookUpType::Mdl(mdl_data) = mdl_data else {
                        warn!("model is not a studio model {}", model_name);
                        return;
                    };

                    entity_textures.insert(model_name.to_string(), get_mdl_textures(&mdl_data));
                }
                EntityModel::Sprite {
                    ref sprite_name, ..
                } => {
                    if entity_textures.contains_key(sprite_name) {
                        return;
                    }

                    let Some(spr_data) = resource.model_lookup.get(sprite_name) else {
                        warn!("cannot get sprite for loading texture {}", sprite_name);
                        return;
                    };

                    let ModelLookUpType::Spr(spr_data) = spr_data else {
                        warn!("model is not a sprite {}", sprite_name);
                        return;
                    };

                    entity_textures.insert(sprite_name.to_string(), get_sprite_textures(&spr_data));
                }
                // for other entities, we don't load texture
                EntityModel::OpaqueEntityBrush(_)
                | EntityModel::TransparentEntityBrush(_)
                | EntityModel::NoDrawBrush(_) => {}
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
                EntityModel::BspMdlEntity { ref model_name, .. } => {
                    Some((model_name, entity.world_index))
                }
                EntityModel::Sprite {
                    ref sprite_name, ..
                } => Some((sprite_name, entity.world_index)),
                // be explicit
                // not being explicit bit me in the ass
                EntityModel::OpaqueEntityBrush(_)
                | EntityModel::TransparentEntityBrush(_)
                | EntityModel::NoDrawBrush(_) => None,
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

// Returns (opaque batch lookup, transparent batch lookup)
fn create_batch_lookups(
    resource: &BspResource,
    // make sure entity info is sorted by world index
    sorted_entity_infos: &[&WorldEntity],
    world_texture_lookup: &WorldTextureLookupTable,
    lightmap: &LightMapAtlasBuffer,
) -> (BatchLookup, BatchLookup, MvpLookup) {
    let mut opaque_lookup = BatchLookup::new();
    let mut transparent_lookup = BatchLookup::new();
    let mut mvp_lookup: HashMap<usize, usize> = HashMap::new();

    let bsp = &resource.bsp;

    // the indices for the skeletal bones start right after all entities
    // for bone index 0, it uses the entity index
    // for bone index 1 and so on, it uses `current_bsp_model_skeletal_bone_mvp_idx`
    // this makes the shader less complicated
    let mut current_bsp_model_skeletal_bone_mvp_idx = sorted_entity_infos.len();

    sorted_entity_infos.iter().for_each(|entity| {
        let world_entity_index = entity.world_index;

        let is_transparent = matches!(
            entity.model,
            EntityModel::TransparentEntityBrush(_) | EntityModel::Sprite { .. }
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
            | EntityModel::NoDrawBrush(_) => {
                let bsp_model_index = match &entity.model {
                    EntityModel::Bsp => 0,
                    EntityModel::OpaqueEntityBrush((x, _)) => *x,
                    EntityModel::TransparentEntityBrush((x, _)) => *x,
                    EntityModel::NoDrawBrush(x) => *x,
                    _ => unreachable!("cannot get bsp model index out of this model"),
                };

                let model = &bsp.models[bsp_model_index as usize];

                let first_face = model.first_face as usize;
                let face_count = model.face_count as usize;

                let faces = &bsp.faces[first_face..(first_face + face_count)];

                let is_nodraw = matches!(entity.model, EntityModel::NoDrawBrush(_));

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
                        } else if is_nodraw {
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
            EntityModel::BspMdlEntity {
                model_name,
                submodel,
                ..
            } => {
                // REMINDER: at the end of this scope, need to increment the bone number
                let Some(mdl) = resource.model_lookup.get(model_name) else {
                    warn!("Cannot find model `{model_name}` to create a batch lookup");

                    return;
                };

                let ModelLookUpType::Mdl(mdl) = mdl else {
                    warn!("Model `{}` is not a studio model", model_name);
                    return;
                };

                create_world_model_vertices(
                    mdl,
                    *submodel,
                    world_entity_index,
                    world_texture_lookup,
                    assigned_lookup,
                    1,
                    |bone_idx| {
                        if bone_idx == 0 {
                            world_entity_index as u32
                        } else {
                            (current_bsp_model_skeletal_bone_mvp_idx + bone_idx as usize - 1) as u32
                        }
                    },
                );

                // only add if bone length is greater than 1
                // otherwise it is jsut a single bone
                // this make the data cleaner
                if mdl.bones.len() > 1 {
                    mvp_lookup.insert(world_entity_index, current_bsp_model_skeletal_bone_mvp_idx);
                }

                // REMINDER: add the bone idx for the current model
                // need to sub 1 because one bone is in another buffer
                // make sure it is saturating sub just in case someone made a model with 0 bone
                current_bsp_model_skeletal_bone_mvp_idx += mdl.bones.len().saturating_sub(1);
            }
            EntityModel::Sprite {
                sprite_name,
                custom_render,
                frame_rate,
            } => {
                let Some(spr) = resource.model_lookup.get(sprite_name) else {
                    warn!("Cannot find sprite `{sprite_name}` to create a batch lookup");

                    return;
                };

                let ModelLookUpType::Spr(spr) = spr else {
                    warn!("Model `{}` is not a sprite model", sprite_name);
                    return;
                };

                let (array_idx, layer_idx) = world_texture_lookup
                    // start with the texture 0 and then the shader will animate the texture
                    .get(&(world_entity_index, 0))
                    .expect("cannot get world texture");

                // V0------V1
                // |      / |
                // |    0   |
                // |  /     |
                // V2------V3

                let half_width = spr.header.max_width as f32 / 2.;
                let half_height = spr.header.max_height as f32 / 2.;

                let v0 = ([-half_width, half_height], [0., 0.]);
                let v1 = ([half_width, half_height], [1., 0.]);
                let v2 = ([-half_width, -half_height], [0., 1.]);
                let v3 = ([half_width, -half_height], [1., 1.]);

                let frame_count = spr.frames.len() as u32;
                let orientation_type = spr.header.orientation as u32;
                let packed_frame_orientation = frame_count << 16 | orientation_type;

                let vertices: Vec<WorldVertex> = [v0, v1, v2, v3]
                    .into_iter()
                    .map(|(pos, uv)| WorldVertex {
                        pos: [pos[0], pos[1], 0.],
                        tex_coord: uv,
                        normal: [0.; 3],
                        layer: *layer_idx as u32,
                        type_: WorldVertexType::Sprite.into(),
                        data_a: [*frame_rate, 0., custom_render.renderamt],
                        data_b: [
                            custom_render.rendermode as u32,
                            world_entity_index as u32,
                            packed_frame_orientation,
                        ],
                    })
                    .collect();
                let indices = [0, 1, 2, 2, 1, 3];

                let batch = assigned_lookup
                    .entry(*array_idx)
                    .or_insert((Vec::new(), Vec::new()));

                let new_vertices_offset = batch.0.len();

                batch.0.extend(vertices);
                batch
                    .1
                    .extend(indices.into_iter().map(|i| i + new_vertices_offset as u32));
            }
        };
    });

    (opaque_lookup, transparent_lookup, mvp_lookup)
}
