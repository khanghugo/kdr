use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use cgmath::{Rad, Rotation3, Zero};
use image::RgbaImage;

use crate::renderer::utils::{
    BspAngles, build_mvp_from_origin_angles, get_idle_sequence_origin_angles,
};

// MUST BE uppercase
pub const NO_RENDER_TEXTURE: &[&str] = &["NULL", "CLIP", "SKY"];

pub enum EntityModel {
    // Entity brushes could be grouped into Bsp if we want some optimization.
    // The only case when EntityBrush type is used is when EntityBrush has its own MVP in the cases of func_rotating_door and alikes.
    Bsp,
    /// Data inside is Bsp Model Index
    OpaqueEntityBrush((i32, CustomRender)),
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
    // [`bsp::Bsp`] is not encapsulated by [`ModelType`] enum is because this is more convenient.
    pub bsp: bsp::Bsp,
    pub entity_dictionary: EntityDictionary,
    // Make up the order until it works
    pub skybox: Vec<RgbaImage>,
}

pub struct WorldEntity {
    /// World index based on the world aka current render context, not BSP.
    ///
    /// It is used for correctly allocating data without going overboard.
    pub world_index: usize,
    pub model: EntityModel,
    pub origin: [f32; 3],
    /// Roll Pitch Yaw (XYZ)
    pub angles: cgmath::Quaternion<f32>,
}

impl WorldEntity {
    fn worldspawn() -> Self {
        Self {
            world_index: 0,
            model: EntityModel::Bsp,
            origin: [0f32; 3],
            angles: cgmath::Quaternion::zero(),
        }
    }

    pub fn build_mvp(&self) -> cgmath::Matrix4<f32> {
        build_mvp_from_origin_angles(self.origin, self.angles)
    }
}

const MODEL_ENTITIES: &[&str] = &["cycler_sprite", "env_sprite"];

// all of the resources to feed to the render context
pub fn get_bsp_resources(bsp: bsp::Bsp, bsp_path: &Path) -> BspResource {
    let mut res = BspResource {
        bsp,
        entity_dictionary: EntityDictionary::new(),
        skybox: vec![],
    };

    let mut gamemod_folder = bsp_path;
    let mut can_load_game_assets = true;

    if let Some(bsp_folder) = bsp_path.parent() {
        if let Some(_gamemod_folder) = bsp_folder.parent() {
            gamemod_folder = _gamemod_folder;
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
            let bsp_angles = BspAngles(
                entity
                    .get("angles")
                    .and_then(|angles| vec3(angles))
                    .unwrap_or(VEC3_ZERO),
            );

            let angles = bsp_angles.get_world_angles();

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

                // "angles" key only works on model, not world/entity brush
                // it is reserved for the like of func_door and alikes
                let angles = cgmath::Quaternion::zero();

                if is_opaque {
                    res.entity_dictionary.insert(
                        bsp_entity_index,
                        WorldEntity {
                            world_index: assign_world_index(),
                            model: EntityModel::OpaqueEntityBrush((
                                bsp_model_index,
                                CustomRender {
                                    rendermode,
                                    renderamt,
                                    renderfx,
                                },
                            )),
                            origin,
                            angles,
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
                        },
                    );
                }

                return;
            }

            if is_mdl && can_load_game_assets {
                let model_path = gamemod_folder.join(model_path);

                let Ok(mdl) = mdl::Mdl::open_from_file(model_path.as_path()) else {
                    println!("cannot open .mdl `{}`", model_path.display());
                    return;
                };

                let (idle_origin, idle_angles) = get_idle_sequence_origin_angles(&mdl);
                let idle_angles = idle_angles.get_world_angles();

                let new_origin = [
                    origin[0] + idle_origin[0],
                    origin[1] + idle_origin[1],
                    origin[2] + idle_origin[2],
                ];

                let bsp_angles = [
                    Rad(angles[0].to_radians()),
                    Rad(angles[1].to_radians()),
                    Rad(angles[2].to_radians()),
                ];
                let mdl_angles = [
                    Rad(idle_angles[0]),
                    Rad(idle_angles[1]),
                    Rad(idle_angles[2]),
                ];

                // dont touch this, this is the order, it has to be like that
                let new_angles = cgmath::Quaternion::from_angle_z(mdl_angles[2] + bsp_angles[2])
                    * cgmath::Quaternion::from_angle_y(mdl_angles[1] + bsp_angles[1])
                    * cgmath::Quaternion::from_angle_x(mdl_angles[0] + bsp_angles[0]);

                res.entity_dictionary.insert(
                    bsp_entity_index,
                    WorldEntity {
                        world_index: assign_world_index(),
                        model: EntityModel::Mdl(mdl),
                        origin: new_origin,
                        angles: new_angles,
                    },
                );
            }

            // TODO
            if is_sprite && can_load_game_assets {
                42;
            }
        });

    // loading skybox now
    if can_load_game_assets {
        let entity0 = &res.bsp.entities[0];
        let skyname = entity0
            .get("skyname")
            .map(|f| f.to_owned())
            .unwrap_or("desert".to_string());

        // skybox order here, already works.
        // need to make sure shader coordinate is flipped accordingly and culling mode is right
        const SUFFIXES: &[&str] = &["ft", "bk", "up", "dn", "rt", "lf"];

        // even though it is file name, it can also be path inside another folder
        let file_names: Vec<String> = SUFFIXES
            .iter()
            .map(|suffix| format!("{}{}.tga", skyname, suffix))
            .collect();

        let paths: Vec<PathBuf> = file_names
            .iter()
            .map(|file_name| gamemod_folder.join("gfx/env").join(file_name))
            .filter_map(|path| proper_file_searching(path.as_path()))
            .collect();

        if paths.len() != 6 {
            println!("cannot load skyboxes because the program cannot find any");
        } else {
            let images: Vec<_> = paths
                .iter()
                .filter_map(|path| image::open(path).ok())
                .collect();

            if images.len() != 6 {
                println!("cannot open all images");
            } else {
                res.skybox.extend(images.into_iter().map(|i| i.to_rgba8()));
            }
        }
    }

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

// the file path must start from gamemod and it shouldnt have anything in prefix except for gamemod
// eg: cstrike/maps/de_dust2 works
// eg: ./cstrike/maps/de_dust2 does not work
fn proper_file_searching(file_path: &Path) -> Option<PathBuf> {
    if file_path.exists() {
        return file_path.to_path_buf().into();
    }

    let components: Vec<_> = file_path.components().collect();

    let Component::Normal(gamemod_name) = components.get(0)? else {
        return None;
    };

    let gamemod_name = gamemod_name.to_str()?;
    let is_download = gamemod_name.ends_with("downloads");

    let mut gamemods_to_check = vec![];

    // cannot search other folder, i guess?
    if gamemod_name == "valve" {
        return None;
    }

    gamemods_to_check.push("valve");

    if is_download {
        gamemods_to_check.push("cstrike");
    } else {
        gamemods_to_check.push("cstrike_downloads");
    }

    let without_gamemod: PathBuf = components[1..].iter().collect();

    for gamemod_to_check in gamemods_to_check {
        let new_path = Path::new(gamemod_to_check).join(without_gamemod.as_path());

        if new_path.exists() {
            return Some(new_path);
        }
    }

    None
}
