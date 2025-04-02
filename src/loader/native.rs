use std::path::{Component, Path, PathBuf};

use bsp::Bsp;

use crate::{
    err,
    loader::{MODEL_ENTITIES, ResourceMap},
};

use super::{ResourceProvider, SKYBOX_SUFFIXES, fix_bsp_file_name};

/// Lots of extra work but it is worth it
pub struct NativeResourceProvider {
    /// Path to the game directory aka /path/to/hl.exe
    ///
    /// This data should be provided so that a demo can be played regardless of wherever it is on the drive.
    game_dir: PathBuf,
}

impl NativeResourceProvider {
    pub fn new(game_dir: impl AsRef<Path>) -> Self {
        Self {
            game_dir: game_dir.as_ref().to_path_buf(),
        }
    }
}

impl ResourceProvider for NativeResourceProvider {
    // TODO better error handling
    // we can do "missing skybox file", "cannot find file", "ill configured server", "blah blah blah"
    /// This step is sort of repetitive because we can just load data in the bsp_resource step instead
    ///
    /// But whatever
    ///
    /// Funny enough, the server processing side in the future would use this portion of code to fetch data.
    ///
    /// So this function will be refactored or maybe just straight up used in the wrong context.
    async fn get_resource(
        &self,
        identifier: &super::ResourceIdentifier,
    ) -> eyre::Result<super::Resource> {
        let map_name = fix_bsp_file_name(identifier.map_name.as_str());
        let map_relative_path = PathBuf::from("maps").join(map_name.as_str());

        // need to properly search the bsp as well
        let Some(path_to_map) = search_game_resource(
            &self.game_dir,
            &identifier.game_mod,
            map_relative_path.as_path(),
        ) else {
            return err!("cannot find .bsp `{}`", map_name);
        };

        let bsp = Bsp::from_file(path_to_map.as_path())?;

        let mut resource_map = ResourceMap::new();

        // need to find .mdl and .spr, for now. maybe sound in the future
        for entity in bsp.entities.iter() {
            let is_model_entity = entity
                .get("classname")
                .map(|classname| MODEL_ENTITIES.contains(&classname.as_str()))
                .unwrap_or(false);

            if !is_model_entity {
                continue;
            }

            let Some(model_path) = entity.get("model") else {
                continue;
            };

            // just to make sure
            if !model_path.ends_with(".mdl") {
                continue;
            }

            let Some(model_absolute_path) =
                search_game_resource(&self.game_dir, &identifier.game_mod, Path::new(model_path))
            else {
                println!("cannot find model `{model_path}`");
                continue;
            };

            let Ok(model_bytes) = std::fs::read(model_absolute_path.as_path()) else {
                println!("cannot load model {}", model_absolute_path.display());
                continue;
            };

            resource_map.insert(model_path.to_string(), model_bytes);
        }

        // get the skybox, just for the bsp_resource to do the same thing again
        {
            // TODO find the skybox from the resources instead of deducing stuffs here
            let entity0 = &bsp.entities[0];
            let skyname = entity0
                .get("skyname")
                .map(|f| f.to_owned())
                .unwrap_or("desert".to_string());

            // even though it is file name, it can also be path inside another folder
            let file_names: Vec<String> = SKYBOX_SUFFIXES
                .iter()
                .map(|suffix| format!("{}{}.tga", skyname, suffix))
                .collect();

            // first, have local path so that we can send to the client
            let local_paths: Vec<PathBuf> = file_names
                .iter()
                .map(|file_name| PathBuf::from("gfx/env").join(file_name))
                // .filter_map(|path| proper_file_searching(path.as_path()))
                .collect();

            // then have the actual path of the skybox, starting from the gamedir so that we can load the images
            // the reason why this is needed is because the skybox might be in "valve" folder instead
            // the relative path can stay the same but we need the absolute path to open the correct file
            let absolute_paths: Vec<PathBuf> = local_paths
                .iter()
                .filter_map(|path| search_game_resource(&self.game_dir, &identifier.game_mod, path))
                .collect();

            if absolute_paths.len() == 6 {
                let images: Vec<_> = absolute_paths
                    .iter()
                    .filter_map(|path| std::fs::read(path).ok())
                    .collect();

                if images.len() == 6 {
                    local_paths
                        .iter()
                        .zip(images.into_iter())
                        .for_each(|(path, image_bytes)| {
                            resource_map.insert(path.display().to_string(), image_bytes);
                        });
                }
            }
        }

        Ok(super::Resource {
            bsp,
            resources: resource_map,
        })
    }
}

// the file path must start from gamemod and it shouldnt have anything in prefix except for gamemod
// eg: cstrike/maps/de_dust2 works
// eg: ./cstrike/maps/de_dust2 does not work
fn search_game_resource(game_dir: &Path, game_mod: &str, relative_path: &Path) -> Option<PathBuf> {
    let one_shot_path = game_dir.join(game_mod).join(relative_path);

    if one_shot_path.exists() {
        return one_shot_path.into();
    }

    let is_valve = game_mod == "valve";
    let is_download = game_mod.ends_with("downloads");
    let mut gamemods_to_check: Vec<String> = vec![];

    // if someone feeds in half life maps, check for valve_downloads because why not
    // otherwise, add valve to our list
    if is_valve {
        gamemods_to_check.push("value_downloads".to_string());
    } else {
        // every else needs to check in with "valve"
        gamemods_to_check.push("valve".to_string());
        gamemods_to_check.push("value_downloads".to_string());

        if is_download {
            let without_download = game_mod.replace("_downloads", "");

            gamemods_to_check.push(without_download);
        } else {
            gamemods_to_check.push(format!("{game_mod}_downloads"));
        }
    }

    for gamemod_to_check in gamemods_to_check {
        let new_path = game_dir.join(gamemod_to_check).join(relative_path);

        if new_path.exists() {
            return Some(new_path);
        }
    }

    None
}
