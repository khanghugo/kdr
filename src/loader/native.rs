use std::path::{Component, Path, PathBuf};

use bsp::Bsp;

use crate::loader::{MODEL_ENTITIES, ResourceMap};

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

        let game_mod = self.game_dir.join(identifier.game_mod.as_str());
        let path_to_map = game_mod.join("maps").join(map_name);
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

            let model_real_path = game_mod.join(model_path);
            let Ok(model_bytes) = std::fs::read(model_real_path.as_path()) else {
                println!("cannot load model {}", model_real_path.display());
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
                .filter_map(|path| proper_file_searching(game_mod.join(path).as_path()))
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
