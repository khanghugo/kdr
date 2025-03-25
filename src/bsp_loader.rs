use std::{collections::HashMap, path::Path};

use cgmath::SquareMatrix;

pub enum EntityModel {
    // Entity brushes could be grouped into Bsp if we want some optimization.
    // The only case when EntityBrush type is used is when EntityBrush has its own MVP in the cases of func_rotating_door and alikes.
    Bsp,
    /// Data inside is Bsp Model Index
    OpaqueEntityBrush(i32),
    /// Data inside is Bsp Model Index, Custom Render properties
    TransparentEntityBrush((i32, CustomRender)),
    // For convenient sake, store the model inside this.
    Mdl(mdl::Mdl),
    // TODO: implement sprite loading, sprite will likely be inside transparent buffer
    Sprite,
}

pub struct CustomRender {
    pub rendermode: i32,
    pub renderamt: f32,
    pub renderfx: i32,
}

/// Key: Bsp Entity Index
/// Value: Entity info
pub type EntityDictionary = HashMap<usize, WorldEntity>;

pub struct BspResource {
    // [`bsp::Bsp`] is not encapsulated by [`ModelType`] enum is because this is more convenient
    pub bsp: bsp::Bsp,
    pub entity_dictionary: EntityDictionary,
}

pub struct WorldEntity {
    /// World index based on the world aka current render context, not BSP.
    ///
    /// It is used for correctly allocating data without going overboard.
    pub world_index: usize,
    pub model: EntityModel,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub model_view_projection: cgmath::Matrix4<f32>,
}

impl WorldEntity {
    fn worldspawn() -> Self {
        Self {
            world_index: 0,
            model: EntityModel::Bsp,
            origin: [0f32; 3],
            angles: [0f32; 3],
            model_view_projection: cgmath::Matrix4::identity(),
        }
    }
}

const MODEL_ENTITIES: &[&str] = &["cycler_sprite", "env_sprite"];

// all of the resources to feed to the render context
pub fn get_bsp_resources(bsp: bsp::Bsp, bsp_path: &Path) -> BspResource {
    let mut res = BspResource {
        bsp,
        entity_dictionary: EntityDictionary::new(),
    };

    let mut gamedir = bsp_path;
    let mut can_load_game_assets = true;

    if let Some(bsp_folder) = bsp_path.parent() {
        if let Some(gamedir_folder) = bsp_folder.parent() {
            gamedir = gamedir_folder;
        } else {
            can_load_game_assets = false;
        }
    } else {
        can_load_game_assets = false;
    };

    let mut available_world_index = 0;
    let mut assign_world_index = move || {
        let value = available_world_index;
        available_world_index += 1;
        value
    };

    res.bsp
        .entities
        .iter()
        .enumerate()
        .for_each(|(bsp_entity_index, entity)| {
            // just add worlspawn if it is 0
            if bsp_entity_index == 0 {
                // for some reason, incrementing available_world_index doesn't work
                res.entity_dictionary
                    .insert(assign_world_index(), WorldEntity::worldspawn());
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

            let is_entity_brush = model_path.starts_with("*");
            let is_valid_model_displaying_entity = MODEL_ENTITIES.contains(&classname.as_str());
            let is_sprite = !is_entity_brush
                && is_valid_model_displaying_entity
                && model_path.ends_with(".spr");
            let is_mdl = !is_entity_brush
                && !is_sprite
                && is_valid_model_displaying_entity
                && model_path.ends_with(".mdl");

            let origin = entity
                .get("origin")
                .and_then(|origin| vec3(origin))
                .unwrap_or(VEC3_ZERO);
            let angles = entity
                .get("angles")
                .and_then(|angles| vec3(angles))
                .unwrap_or(VEC3_ZERO);
            let model_view_projection_matrix = cgmath::Matrix4::from_translation(origin.into())
                * cgmath::Matrix4::from_angle_x(cgmath::Deg(angles[0]))
                * cgmath::Matrix4::from_angle_y(cgmath::Deg(angles[1]))
                * cgmath::Matrix4::from_angle_z(cgmath::Deg(angles[2]));

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

                if is_opaque {
                    res.entity_dictionary.insert(
                        bsp_entity_index,
                        WorldEntity {
                            world_index: assign_world_index(),
                            model: EntityModel::OpaqueEntityBrush(bsp_model_index),
                            origin,
                            angles,
                            model_view_projection: model_view_projection_matrix,
                        },
                    );
                } else {
                    res.entity_dictionary.insert(
                        bsp_entity_index,
                        WorldEntity {
                            world_index: assign_world_index(),
                            model: EntityModel::TransparentEntityBrush((
                                bsp_model_index,
                                CustomRender {
                                    rendermode,
                                    renderamt,
                                    renderfx,
                                },
                            )),
                            origin,
                            angles,
                            model_view_projection: model_view_projection_matrix,
                        },
                    );
                }

                return;
            }

            if is_mdl && can_load_game_assets {
                let model_path = gamedir.join(model_path);

                let Ok(mdl) = mdl::Mdl::open_from_file(model_path.as_path()) else {
                    println!("cannot open .mdl `{}`", model_path.display());
                    return;
                };

                res.entity_dictionary.insert(
                    bsp_entity_index,
                    WorldEntity {
                        world_index: assign_world_index(),
                        model: EntityModel::Mdl(mdl),
                        origin,
                        angles,
                        model_view_projection: model_view_projection_matrix,
                    },
                );
            }

            // TODO
            if is_sprite && can_load_game_assets {
                42;
            }
        });

    return res;
}

const VEC3_ZERO: [f32; 3] = [0f32; 3];

fn vec3(i: &str) -> Option<[f32; 3]> {
    let res: Vec<f32> = i
        .split_whitespace()
        .filter_map(|n| n.parse::<f32>().ok())
        .collect();

    if res.len() < 3 {
        return None;
    }

    Some([res[0], res[1], res[2]])
}
