//! At this step, the user already has all of the assets related to a replay.
//!
//! Now, they process all of them and feed that into the renderer context.

use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
};

use cgmath::{Rad, Rotation3, VectorSpace};
use common::{
    BspAngles, MdlPosRot, NO_DRAW_FUNC_BRUSHES, PosRot, build_mvp_from_pos_and_rot,
    model_to_world_transformation, origin_posrot, setup_studio_model_transformations, vec3,
};
use image::RgbaImage;
use kira::sound::static_sound::StaticSoundData;
use mdl::{Mdl, SequenceFlag};
use tracing::warn;
use wad::types::Wad;

use crate::MODEL_ENTITIES;

use super::{Resource, SKYBOX_SUFFIXES};

/// Key: Bsp Entity Index
///
/// Value: Entity info
pub type EntityDictionary = HashMap<usize, WorldEntity>;

pub enum EntityModel {
    // Entity brushes could be grouped into Bsp if we want some optimization.
    // The only case when EntityBrush type is used is when EntityBrush has its own MVP in the cases of func_rotating_door and alikes.
    Bsp,
    /// Data inside is Bsp Model Index
    OpaqueEntityBrush((i32, CustomRender)),
    /// Data inside is Bsp Model Index, Custom Render properties
    TransparentEntityBrush((i32, CustomRender)),
    /// This is for all entity brushes that shouldn't be rendered. That includes all trigger_ and some func_
    /// such as func_ladder or func_hostage_rescue. Why are they "func" but not "trigger"???
    ///
    /// These brushes don't have lightmap so they will be black.
    /// In addition, they will have renderamt 0. This causes hall of mirror effect on GLES.
    NoDrawBrush(i32),
    // Data stored inside is the model name to get it from the `models` hash map inside [`BspResource`].
    BspMdlEntity {
        model_name: String,
        /// submodel
        submodel: usize,
    },
    // TODO: implement sprite loading, sprite will likely be inside transparent buffer
    Sprite,
}

pub struct CustomRender {
    pub rendermode: i32,
    pub renderamt: f32,
    pub renderfx: i32,
}

pub struct WorldTransformationSkeletal {
    pub current_sequence_index: usize,
    // storing base world transformation
    pub world_transformation: PosRot,
    // storing model transformation on top of that
    pub model_transformations: MdlPosRot,
    // data related to each model transformation
    pub model_transformation_infos: Vec<ModelTransformationInfo>,
}

pub type WorldTransformationEntity = PosRot;

pub enum WorldTransformation {
    /// For entity brushes, they only have one transformation, so that is good.
    Entity(PosRot),
    /// For skeletal system, multiple transformations means there are multiple bones.
    ///
    /// So, we store all bones transformation and then put it back in shader when possible.
    ///
    /// And we also store all information related to the model. Basically a lite mdl format
    Skeletal(WorldTransformationSkeletal),
}

impl WorldTransformation {
    fn worldspawn() -> Self {
        Self::Entity(origin_posrot())
    }

    pub fn get_entity(&self) -> &WorldTransformationEntity {
        match self {
            WorldTransformation::Entity(x) => x,
            WorldTransformation::Skeletal(_) => unreachable!(),
        }
    }

    pub fn get_skeletal_mut(&mut self) -> &mut WorldTransformationSkeletal {
        match self {
            WorldTransformation::Entity(_) => unreachable!(),
            WorldTransformation::Skeletal(x) => x,
        }
    }
}

pub struct ModelTransformationInfo {
    pub frame_per_second: f32,
    pub looping: bool,
}

pub struct WorldEntity {
    /// World index based on the world aka current render context, not BSP.
    ///
    /// It is used for correctly allocating data without going overboard.
    pub world_index: usize,
    pub model: EntityModel,
    /// The reason why it is a vector is so that we can have an entity with multiple transformation based on its vertices.
    ///
    /// This seems like a hack to retrofit mdl skeletal system. It does.
    pub transformation: WorldTransformation,
}

pub enum BuildMvpResult {
    Entity(cgmath::Matrix4<f32>),
    Skeletal(Vec<cgmath::Matrix4<f32>>),
}

impl WorldEntity {
    fn worldspawn() -> Self {
        Self {
            world_index: 0,
            model: EntityModel::Bsp,
            transformation: WorldTransformation::worldspawn(),
        }
    }

    pub fn build_mvp(&self, time: f32) -> BuildMvpResult {
        match &self.transformation {
            WorldTransformation::Entity((position, rotation)) => {
                BuildMvpResult::Entity(build_mvp_from_pos_and_rot(*position, *rotation))
            }
            WorldTransformation::Skeletal(WorldTransformationSkeletal {
                current_sequence_index,
                world_transformation: (world_pos, world_rot),
                model_transformations,
                model_transformation_infos,
            }) => {
                // quick hack to avoid some weird timing issue,
                // TODO: reset the sequence on timeline scrub
                let current_sequence_index =
                    (*current_sequence_index).min(model_transformations.len() - 1);

                let current_sequence = &model_transformations[current_sequence_index];
                let current_sequence_info = &model_transformation_infos[current_sequence_index];

                // TODO blending
                let current_blend = &current_sequence[0];
                let frame_count = current_blend.len();

                // TODO start time
                let anim_total_time = frame_count as f32 / current_sequence_info.frame_per_second;
                let anim_time = if current_sequence_info.looping {
                    time % anim_total_time
                } else {
                    time
                };
                let anim_frame_from_idx =
                    ((anim_time * current_sequence_info.frame_per_second as f32).floor() as usize)
                        .min(frame_count - 1);

                // usually, the first condition will never hit, but whatever
                if anim_frame_from_idx == frame_count - 1 {
                    let anim_frame = &current_blend[anim_frame_from_idx];

                    let transformations = anim_frame
                        .iter()
                        .map(|&posrot| {
                            let (pos, rot) =
                                model_to_world_transformation(posrot, *world_pos, *world_rot);

                            build_mvp_from_pos_and_rot(pos, rot)
                        })
                        .collect();

                    BuildMvpResult::Skeletal(transformations)
                } else {
                    let anim_frame_to_idx = anim_frame_from_idx + 1;

                    let from_frame = &current_blend[anim_frame_from_idx];
                    let to_frame = &current_blend[anim_frame_to_idx];

                    let target = (anim_time * current_sequence_info.frame_per_second).fract();

                    let transformations = from_frame
                        .iter()
                        .zip(to_frame.iter())
                        .map(|((from_pos, from_rot), (to_pos, to_rot))| {
                            let lerped_posrot = (
                                from_pos.lerp(*to_pos, target),
                                from_rot.nlerp(*to_rot, target),
                            );

                            let (pos, rot) = model_to_world_transformation(
                                lerped_posrot,
                                *world_pos,
                                *world_rot,
                            );

                            build_mvp_from_pos_and_rot(pos, rot)
                        })
                        .collect();

                    BuildMvpResult::Skeletal(transformations)
                }
            }
        }
    }
}

/// Holds all data related to BSP for rendering. The client will process the data for the renderer to use.
///
/// This struct must acquires all data from [`loader::Resource`].
pub struct BspResource {
    // [`bsp::Bsp`] is not encapsulated by [`ModelType`] enum is because this is more convenient.
    pub bsp: bsp::Bsp,
    pub entity_dictionary: EntityDictionary,
    // Make up the order until it works
    pub skybox: Vec<RgbaImage>,
    // Some maps use external WAD textures.
    // In native implementation, we can load multiple wad files just fine because client processing is cheap.
    // But in web implementation, we have to make sure we only have 1 wad texture.
    // This means, we have to pre-process all BSP files to have their own wad file if needed.
    pub external_wad_textures: Vec<Wad>,
    // All model entities point here to reuse the model data. With this, we won't have duplicated texture data.
    // There is still duplicated vertex data though but those are cheaper than textures.
    // Key is model path. Value is model data.
    pub model_lookup: HashMap<String, mdl::Mdl>,
    // Similar to how model look up works. This time, we nicely have an abstract sound data type (looking at rodio with type erasure)
    pub sound_lookup: HashMap<String, StaticSoundData>,
}

impl BspResource {
    // Must acquire resource because we are gonna use all of them.
    // It is pretty nice that this step won't fail because we can just ignore errors (but there aren't any errors).
    pub fn new(resource: Resource) -> Self {
        let mut entity_dictionary = EntityDictionary::new();
        let mut model_lookup = HashMap::new();
        let mut skybox = vec![];
        let mut sound_lookup = HashMap::new();

        load_world_entities(&resource, &mut entity_dictionary, &mut model_lookup);
        // load_viewmodels(&resource, &mut entity_dictionary, &mut model_lookup);
        // load_player_models(&resource, &mut entity_dictionary, &mut model_lookup);

        load_skybox(&resource, &mut skybox);

        // find external wad files
        let external_wad_textures: Vec<Wad> = resource
            .resources
            .iter()
            .filter(|(k, _)| k.ends_with(".wad"))
            .map(|(_, wad_bytes)| Wad::from_bytes(wad_bytes))
            .collect::<eyre::Result<Vec<_>>>() // cool new thing i learned
            .expect("cannot load all wad files");

        load_sound(&resource, &mut sound_lookup);

        BspResource {
            bsp: resource.bsp,
            entity_dictionary,
            skybox,
            external_wad_textures,
            model_lookup,
            sound_lookup,
        }
    }
}

fn load_world_entities(
    resource: &Resource,
    entity_dictionary: &mut EntityDictionary,
    model_lookup: &mut HashMap<String, mdl::Mdl>,
) {
    let mut available_world_index = 0;
    let mut assign_world_index = move || {
        let value = available_world_index;
        available_world_index += 1;
        value
    };

    resource
        .bsp
        .entities
        .iter()
        .enumerate()
        .for_each(|(bsp_entity_index, entity)| {
            // just add worlspawn if it is 0
            if bsp_entity_index == 0 {
                // for some reason, incrementing available_world_index doesn't work
                entity_dictionary.insert(assign_world_index(), WorldEntity::worldspawn());
                return;
            }

            // not a valid entity to check
            // exit fast
            let Some(classname) = entity.get("classname") else {
                return;
            };

            // not an entity with a model, not worth checking
            let Some(model_path) = entity.get("model") else {
                return;
            };

            let is_nodraw = is_nodraw(classname);
            let is_entity_brush = model_path.starts_with("*");
            let is_valid_model_displaying_entity = MODEL_ENTITIES.contains(&classname.as_str());
            let is_sprite = !is_entity_brush
                && is_valid_model_displaying_entity
                && model_path.ends_with(".spr");
            let is_mdl = !is_entity_brush
                && !is_sprite
                && is_valid_model_displaying_entity
                && model_path.ends_with(".mdl");

            let entity_world_position: cgmath::Vector3<f32> = entity
                .get("origin")
                .and_then(|origin| vec3(origin))
                .unwrap_or(VEC3_ZERO)
                .into();

            let entity_bsp_angles = BspAngles(
                entity
                    .get("angles")
                    .and_then(|angles| vec3(angles))
                    .unwrap_or(VEC3_ZERO),
            );

            let rendermode = entity
                .get("rendermode")
                .and_then(|rendermode| rendermode.parse::<i32>().ok())
                .unwrap_or(0);
            let renderamt = entity
                .get("renderamt")
                .and_then(|renderamt| renderamt.parse::<f32>().ok())
                .map(|renderamt| {
                    if [0, 4].contains(&rendermode) {
                        255.0
                    } else {
                        renderamt
                    }
                })
                // renderamt is defaulted to be 0.0 everywhere in the code
                // in the case of trigger, they don't have any light map and they aren't even opaque brush
                // that would be handled inside shader instead
                .unwrap_or(0.0);
            let renderfx = entity
                .get("renderfx")
                .and_then(|renderfx| renderfx.parse::<i32>().ok())
                .unwrap_or(0);

            if is_entity_brush {
                let is_opaque = [0, 4].contains(&rendermode) || renderamt == 255.0;
                let bsp_model_index = model_path[1..]
                    .parse::<i32>()
                    .expect("brush model index is not a number");

                // "angles" key only works on model, not world/entity brush
                // it is reserved for the like of func_door and alikes
                // NEED TO USE IDENTITY QUATERNION
                let rotation = origin_posrot().1;

                let normal_opaque_brush = is_opaque && !is_nodraw;
                let normal_transparent_brush = !is_opaque && !is_nodraw;

                let custom_render = CustomRender {
                    rendermode,
                    // make sure that renderamt is 255.0 if opaque
                    // due to some dumb stuffs, this has to be done
                    renderamt: if is_opaque { 255.0 } else { renderamt },
                    renderfx,
                };

                entity_dictionary.insert(
                    bsp_entity_index,
                    WorldEntity {
                        world_index: assign_world_index(),
                        model: if normal_opaque_brush {
                            EntityModel::OpaqueEntityBrush((bsp_model_index, custom_render))
                        } else if normal_transparent_brush {
                            EntityModel::TransparentEntityBrush((bsp_model_index, custom_render))
                        } else if is_nodraw {
                            EntityModel::NoDrawBrush(bsp_model_index)
                        } else {
                            unreachable!("trying to add an unknown brush type: {}", classname)
                        },
                        transformation: WorldTransformation::Entity((
                            entity_world_position,
                            rotation,
                        )),
                    },
                );

                return;
            }

            if is_mdl {
                let is_model_loaded = model_lookup.contains_key(model_path);

                // cannot do hashmap.or_insert_with becuase we won't be able to exit
                if !is_model_loaded {
                    let Some(mdl_bytes) = resource.resources.get(model_path) else {
                        warn!("cannot find '{}' from fetched resources", model_path);
                        return;
                    };

                    let Ok(mdl) = mdl::Mdl::open_from_bytes(mdl_bytes) else {
                        warn!("cannot parse model '{}'", model_path);
                        return;
                    };

                    model_lookup.insert(model_path.to_string(), mdl);
                }

                let mdl = model_lookup
                    .get(model_path)
                    // this this should always work
                    .expect("cannot get recently inserted model.");

                let submodel = entity
                    .get("body")
                    .and_then(|body| body.parse::<f32>().ok())
                    .map(|x| x.floor() as usize)
                    .unwrap_or(0);

                // if model has external textures, just stop bothering
                // TODO: work for models with external textures
                if mdl.textures.is_empty() {
                    return;
                }

                let model_transformations = setup_studio_model_transformations(mdl);

                let entity_world_angles = entity_bsp_angles.get_world_angles();
                let entity_world_angles_rad = [
                    Rad(entity_world_angles[0].to_radians()),
                    Rad(entity_world_angles[1].to_radians()),
                    Rad(entity_world_angles[2].to_radians()),
                ];
                let entity_world_rotation =
                    cgmath::Quaternion::from_angle_z(entity_world_angles_rad[2])
                        * cgmath::Quaternion::from_angle_y(entity_world_angles_rad[1])
                        * cgmath::Quaternion::from_angle_x(entity_world_angles_rad[0]);

                let model_transformation_infos: Vec<ModelTransformationInfo> = mdl
                    .sequences
                    .iter()
                    .map(|sequence| ModelTransformationInfo {
                        frame_per_second: sequence.header.fps,
                        looping: sequence.header.flags.contains(SequenceFlag::LOOPING),
                    })
                    .collect();

                entity_dictionary.insert(
                    bsp_entity_index,
                    WorldEntity {
                        world_index: assign_world_index(),
                        model: EntityModel::BspMdlEntity {
                            model_name: model_path.to_string(),
                            submodel: submodel as usize,
                        },
                        transformation: WorldTransformation::Skeletal(
                            WorldTransformationSkeletal {
                                current_sequence_index: 0,
                                world_transformation: (
                                    entity_world_position,
                                    entity_world_rotation,
                                ),
                                model_transformations,
                                model_transformation_infos,
                            },
                        ),
                    },
                );
            }

            // TODO
            if is_sprite {
                42;
            }
        });
}

// fn load_viewmodels(
//     resource: &Resource,
//     entity_dictionary: &mut EntityDictionary,
//     model_lookup: &mut HashMap<String, mdl::Mdl>,
// ) {
//     let available_world_index = entity_dictionary
//         .values()
//         .fold(0, |acc, e| e.world_index.max(acc))
//         + 1;

//     resource
//         .resources
//         .iter()
//         // only concerned about view models
//         .filter(|(k, _)| {
//             let file_path = Path::new(k);
//             let file_name = file_path.file_name().unwrap().to_str().unwrap();

//             if file_name.starts_with("v_") && file_name.ends_with(".mdl") {
//                 true
//             } else {
//                 false
//             }
//         })
//         .enumerate()
//         .for_each(|(viewmodel_index, (model_path, model_bytes))| {
//             // check that model is not loaded
//             let mdl = model_lookup
//                 .entry(model_path.to_string())
//                 .or_insert_with(|| {
//                     // insert model
//                     match Mdl::open_from_bytes(&model_bytes) {
//                         Ok(x) => x,
//                         Err(err) => {
//                             panic!("cannot parse model {}: {}", model_path, err);
//                         }
//                     }
//                 });

//             let model_transformations = setup_studio_model_transformations(&mdl);
//             let model_transformation_infos: Vec<ModelTransformationInfo> = mdl
//                 .sequences
//                 .iter()
//                 .map(|sequence| ModelTransformationInfo {
//                     frame_per_second: sequence.header.fps,
//                     // explicit no loop for all view models
//                     looping: false,
//                 })
//                 .collect();

//             entity_dictionary.insert(
//                 // this doesn't do anything
//                 3000 + viewmodel_index,
//                 WorldEntity {
//                     world_index: available_world_index + viewmodel_index,
//                     model: EntityModel::ViewModel {
//                         model_name: model_path.to_string(),
//                         submodel: 0,
//                         active: false,
//                     },
//                     transformation: WorldTransformation::Skeletal(WorldTransformationSkeletal {
//                         current_sequence_index: 0,
//                         world_transformation: origin_posrot(),
//                         model_transformations,
//                         model_transformation_infos,
//                     }),
//                 },
//             );
//         });
// }

// similar code to `load_viewmodels` with some slight differences
// at the moment, there is only one instance of each model
// the reason is that the player model is included in the world buffer, very sad time
// fn load_player_models(
//     resource: &Resource,
//     entity_dictionary: &mut EntityDictionary,
//     model_lookup: &mut HashMap<String, mdl::Mdl>,
// ) {
//     let available_world_index = entity_dictionary
//         .values()
//         .fold(0, |acc, e| e.world_index.max(acc))
//         + 1;

//     resource
//         .resources
//         .iter()
//         .filter(|(file_name, _)| {
//             if file_name.ends_with(".mdl") && file_name.starts_with("models/player") {
//                 true
//             } else {
//                 false
//             }
//         })
//         // loop over the models endlessly
//         .cycle()
//         .take(32)
//         .enumerate()
//         .for_each(|(model_index, (model_path, model_bytes))| {
//             // check that model is not loaded
//             let mdl = model_lookup
//                 .entry(model_path.to_string())
//                 .or_insert_with(|| {
//                     // insert model
//                     match Mdl::open_from_bytes(&model_bytes) {
//                         Ok(x) => x,
//                         Err(err) => {
//                             panic!("cannot parse model {}: {}", model_path, err);
//                         }
//                     }
//                 });

//             let model_transformations = setup_studio_model_transformations(&mdl);
//             let model_transformation_infos: Vec<ModelTransformationInfo> = mdl
//                 .sequences
//                 .iter()
//                 .map(|sequence| ModelTransformationInfo {
//                     frame_per_second: sequence.header.fps,
//                     looping: sequence.header.flags.contains(SequenceFlag::LOOPING),
//                 })
//                 .collect();

//             entity_dictionary.insert(
//                 // this doesn't do anything
//                 5000 + model_index,
//                 WorldEntity {
//                     world_index: available_world_index + model_index,
//                     model: EntityModel::PlayerModel {
//                         model_name: model_path.to_string(),
//                         submodel: 0,
//                         // no player to start with
//                         player_index: None,
//                     },
//                     transformation: WorldTransformation::Skeletal(WorldTransformationSkeletal {
//                         current_sequence_index: 0,
//                         world_transformation: origin_posrot(),
//                         model_transformations,
//                         model_transformation_infos,
//                     }),
//                 },
//             );
//         });
// }

fn load_skybox(resource: &Resource, skybox: &mut Vec<RgbaImage>) {
    // TODO find the skybox from the resources instead of deducing stuffs here
    let entity0 = &resource.bsp.entities[0];
    let skyname = entity0
        .get("skyname")
        .map(|f| f.to_owned())
        .unwrap_or("desert".to_string());

    // even though it is file name, it can also be path inside another folder
    let file_names: Vec<String> = SKYBOX_SUFFIXES
        .iter()
        .map(|suffix| format!("{}{}.tga", skyname, suffix))
        .collect();

    let paths: Vec<PathBuf> = file_names
        .iter()
        .map(|file_name| PathBuf::from("gfx/env").join(file_name))
        .collect();

    if paths.len() != 6 {
        println!("cannot load skyboxes because the program cannot find any");
    } else {
        let images: Vec<_> = paths
            .iter()
            .map(|path| path.to_str().unwrap())
            .filter_map(|filename| resource.resources.get(filename))
            .filter_map(|image_bytes| {
                image::load_from_memory_with_format(
                    image_bytes,
                    // explicit tga format because `.load_from_memory` doesnt support tga
                    image::ImageFormat::Tga,
                )
                .ok()
            })
            .collect();

        if images.len() != 6 {
            println!("cannot open all images");
        } else {
            skybox.extend(images.into_iter().map(|i| i.to_rgba8()));
        }
    }
}

fn load_sound(resource: &Resource, sound_lookup: &mut HashMap<String, StaticSoundData>) {
    resource.resources.iter().for_each(|(file_path, bytes)| {
        if !file_path.ends_with(".wav") {
            return;
        }

        if sound_lookup.contains_key(file_path) {
            return;
        }

        let cursor = Cursor::new(
            // HOLY
            bytes.to_owned(),
        );

        let Ok(sound_data) = StaticSoundData::from_cursor(cursor) else {
            warn!("Failed to parse audio file: `{}`", file_path);
            return;
        };

        sound_lookup.insert(file_path.to_owned(), sound_data);

        // sound_lookup.insert(file_path.to_owned(), BspSoundData {
        //     data: sound_data,
        //     is_loop: ,
        //     pos: todo!(),
        //     volume: todo!(),
        // });
    });
}

const VEC3_ZERO: [f32; 3] = [0f32; 3];

fn is_nodraw(classname: &str) -> bool {
    let is_trigger = classname.starts_with("trigger_");
    let is_nodraw_func = NO_DRAW_FUNC_BRUSHES.contains(&classname);

    return is_trigger || is_nodraw_func;
}
