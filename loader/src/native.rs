use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use common::{
    COMMON_GAME_MODS, COMMON_RESOURCE_SOUND, RESOURCE_PLAYER_MODELS, RESOURCE_VIEWMODELS,
    UNKNOWN_GAME_MOD,
};
use ghost::get_ghost_blob_from_path;
use tracing::{info, warn};

use bsp::Bsp;
use wad::types::Wad;

use crate::{
    MODEL_ENTITIES, MapIdentifier, MapList, ProgressResourceProvider, ReplayList, ResourceMap,
    SOUND_ENTITIES,
};

use super::{ResourceProvider, SKYBOX_SUFFIXES, error::ResourceProviderError, fix_bsp_file_name};

#[derive(Debug, Clone)]
/// Lots of extra work but it is worth it
pub struct NativeResourceProvider {
    /// Path to the game directory aka /path/to/hl.exe
    ///
    /// This data should be provided so that a demo can be played regardless of wherever it is on the drive.
    pub game_dir: PathBuf,
}

impl NativeResourceProvider {
    pub fn new(game_dir: impl AsRef<Path>) -> Self {
        Self {
            game_dir: game_dir.as_ref().to_path_buf(),
        }
    }
}

// Need to impl it here as well so in our main code, we call the same function
impl ProgressResourceProvider for NativeResourceProvider {
    async fn get_map_with_progress(
        &self,
        identifier: &MapIdentifier,
        _progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<crate::Resource, ResourceProviderError> {
        self.get_map(identifier).await
    }

    async fn get_replay_with_progress(
        &self,
        replay_name: &str,
        _progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<ghost::GhostBlob, ResourceProviderError> {
        self.get_replay(replay_name).await
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
    async fn get_map(
        &self,
        identifier: &MapIdentifier,
    ) -> Result<super::Resource, ResourceProviderError> {
        let map_name = fix_bsp_file_name(identifier.map_name.as_str());
        let map_relative_path = PathBuf::from("maps").join(map_name.as_str());

        info!("Requesting resources for {}", map_name);

        if identifier.game_mod == UNKNOWN_GAME_MOD {
            info!(
                "Encountering `{}` game mod. Will scan all game mods in the config file",
                UNKNOWN_GAME_MOD
            );
        }

        // need to properly search the bsp as well
        let path_to_map = search_game_resource(
            &self.game_dir,
            &identifier.game_mod,
            map_relative_path.as_path(),
            true,
        )
        .ok_or_else(|| ResourceProviderError::CannotFindBsp {
            bsp_name: map_name.to_owned(),
        })?;

        let bsp = Bsp::from_file(path_to_map.as_path()).map_err(|op| {
            ResourceProviderError::CannotParseBsp {
                source: op,
                bsp_name: map_name.to_owned(),
            }
        })?;

        let mut resource_map = ResourceMap::new();

        get_models_and_sprites(
            &mut resource_map,
            &bsp,
            &self.game_dir,
            &identifier.game_mod,
        );

        // if we cant find skybox, that is fine, who cares
        let _ = get_skybox(
            &mut resource_map,
            &bsp,
            &self.game_dir,
            &identifier.game_mod,
        );

        get_external_wads(
            &mut resource_map,
            &bsp,
            &self.game_dir,
            &identifier.game_mod,
            &path_to_map,
        )?;

        get_sound(
            &mut resource_map,
            &bsp,
            &self.game_dir,
            &identifier.game_mod,
        );

        Ok(super::Resource {
            bsp,
            resources: resource_map,
        })
    }

    // TODO: maybe write this into a file
    async fn get_map_list(&self) -> Result<crate::MapList, ResourceProviderError> {
        let mut map_list = MapList::new();

        // TODO: maybe par_iter?
        COMMON_GAME_MODS.iter().for_each(|game_mod| {
            let path = self.game_dir.join(game_mod).join("maps");

            let map_entry = map_list
                .entry(game_mod.to_string())
                .or_insert(HashSet::new());

            // quick exit
            if !path.exists() {
                return;
            }

            let entries = std::fs::read_dir(path).expect("cannot read folder");

            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|entry| entry.extension().is_some() && entry.is_file())
                .filter_map(|entry| {
                    let ext = entry.extension().unwrap();

                    if ext == "bsp" {
                        return Some(entry.file_stem().unwrap().to_str()?.to_string());
                    }

                    None
                })
                .for_each(|map_name| {
                    map_entry.insert(map_name);
                });
        });

        Ok(map_list)
    }

    async fn get_replay_list(&self) -> Result<crate::ReplayList, ResourceProviderError> {
        match scan_folder_for_files(&self.game_dir, &self.game_dir, &["dem"], true) {
            Some(yse) => Ok(yse),
            None => Err(ResourceProviderError::DemoList),
        }
    }

    async fn get_replay(
        &self,
        replay_name: &str,
    ) -> Result<ghost::GhostBlob, ResourceProviderError> {
        let replay_path = self.game_dir.join(replay_name);

        get_ghost_blob_from_path(&replay_path, None)
            .map_err(|op| ResourceProviderError::Ghost { source: op })
    }

    async fn request_common_resource(&self) -> Result<ResourceMap, ResourceProviderError> {
        let mut resource_map = ResourceMap::new();

        get_other_sound(&mut resource_map, &self.game_dir, "unknown");
        get_viewmodel(&mut resource_map, &self.game_dir, "unknown");
        get_player_models(&mut resource_map, &self.game_dir, "unknown");

        Ok(resource_map)
    }
}

// base so that we can only extract the relative path starting from game_dir
pub fn scan_folder_for_files(
    base: &Path,
    curr: &Path,
    formats: &[&str],
    recursive: bool,
) -> Option<ReplayList> {
    let mut replay_list = ReplayList::new();

    let entries = std::fs::read_dir(curr).expect("cannot read folder");

    // i like bfs
    let mut entries_to_visit = vec![];

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .for_each(|entry| {
            if entry.is_dir() {
                entries_to_visit.push(entry);
                return;
            }

            if entry.is_file() && entry.extension().is_some() {
                let ext = entry.extension().unwrap();

                if formats.contains(&ext.to_str().unwrap()) {
                    let replay_path = entry.strip_prefix(base).unwrap().display().to_string();

                    replay_list.push(replay_path);
                }
            }
        });

    if recursive {
        entries_to_visit.into_iter().for_each(|entry| {
            if let Some(extras) = scan_folder_for_files(base, entry.as_path(), formats, recursive) {
                replay_list.extend(extras);
            }
        });
    }

    Some(replay_list)
}

fn get_models_and_sprites(
    resource_map: &mut ResourceMap,
    bsp: &Bsp,
    game_dir: &Path,
    game_mod: &str,
) {
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
        if !(model_path.ends_with(".mdl") || model_path.ends_with(".spr")) {
            continue;
        }

        // check if resource already exists to avoid unnecessary work
        if resource_map.contains_key(model_path) {
            continue;
        }

        let Some(model_absolute_path) =
            search_game_resource(game_dir, game_mod, Path::new(model_path), true)
        else {
            warn!("cannot find model `{model_path}`");
            continue;
        };

        let Ok(model_bytes) = std::fs::read(model_absolute_path.as_path()) else {
            warn!("cannot load model {}", model_absolute_path.display());
            continue;
        };

        resource_map.insert(model_path.to_string(), model_bytes);
    }
}

fn get_skybox(
    resource_map: &mut ResourceMap,
    bsp: &Bsp,
    game_dir: &Path,
    game_mod: &str,
) -> Result<(), ResourceProviderError> {
    // get the skybox, just for the bsp_resource to do the same thing again
    // UPDATE: skybox in cl_skyname is case insensitive
    // that means, when we look for skybox files, we have to be case insensitive as well.
    // eg: cl_skyname = "hello", that means, "Hellodn.tga" and "hellOuP.tga" are all valid.

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
    let absolute_paths = local_paths
        .iter()
        .map(|path| {
            search_game_resource(
                game_dir, game_mod, path, // skybox searching is case insensitive
                false,
            )
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| ResourceProviderError::CannotFindSkyboxTextures)?;

    let images: Vec<_> = absolute_paths
        .iter()
        .map(|path| std::fs::read(path))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|op| ResourceProviderError::IOErrors { source: op })?;

    local_paths
        .iter()
        .zip(images.into_iter())
        .for_each(|(path, image_bytes)| {
            // when inserting back to the resource map, we use the name from cl_skyname
            resource_map.insert(path.display().to_string(), image_bytes);
        });

    Ok(())
}

fn get_external_wads(
    resource_map: &mut ResourceMap,
    bsp: &Bsp,
    game_dir: &Path,
    game_mod: &str,
    path_to_map: &Path,
) -> Result<(), ResourceProviderError> {
    // check if we need external wad
    let textures_in_external_wad: Vec<String> = bsp
        .textures
        .iter()
        .filter_map(|miptex| {
            if miptex.is_external() {
                return miptex.texture_name.get_string_standard().into();
            }

            None
        })
        .collect();

    let res_path = path_to_map.with_extension("res");
    let mut wad_fallback_search = false;

    if !textures_in_external_wad.is_empty() && res_path.exists() {
        // naive check to see if it has .res file and if the .res includes wad.
        // DO NOT TRUST .RES FILE.
        // we should check if .res file contains all of the external textures we want.
        // if it doesn't, have fall back to nuclear option where we check all wad files. VERY BAD.
        // this is why the server should be responsible for making good .res file and per map .wad file.

        let res_file = std::fs::read_to_string(res_path.as_path()).map_err(|op| {
            ResourceProviderError::IOError {
                source: op,
                path: res_path.to_owned(),
            }
        })?;

        info!("Map has external textures and a .res file. Trying to find .wad files.");

        let wad_relative_paths = res_file
            .lines()
            .filter(|line| line.contains(".wad"))
            .map(|wad_relative_path| wad_relative_path.trim())
            .collect::<Vec<_>>();

        // open all the wad files first and then we check it later
        let wad_paths = wad_relative_paths
            .iter()
            .filter_map(|path| search_game_resource(game_dir, game_mod, &Path::new(path), true))
            .collect::<Vec<_>>();

        // opening the files twice, but i don't think it will be a big concern, i hope
        let wad_files_bytes = wad_paths
            .iter()
            .map(|path| std::fs::read(path))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|op| ResourceProviderError::IOErrors { source: op })?;

        // sequential like this so that the server knows what is wrong when something happens
        let mut wad_files = vec![];
        for (bytes, wad_file_name) in wad_files_bytes.iter().zip(wad_relative_paths.iter()) {
            let wad_file =
                Wad::from_bytes(bytes).map_err(|op| ResourceProviderError::CannotParseWad {
                    source: op,
                    wad_name: wad_file_name.to_string(),
                })?;

            wad_files.push(wad_file);
        }

        // we have to verify that all of our external textures are inside the listed wad files
        let mut all_wad_textures = HashSet::new();

        wad_files.iter().for_each(|wad| {
            wad.entries.iter().for_each(|entry| {
                if let Some(miptex) = entry.file_entry.get_mip_tex() {
                    all_wad_textures.insert(miptex.texture_name.get_string_standard());
                }
            });
        });

        let can_find_all_textures_in_listed_wads = textures_in_external_wad
            .iter()
            .all(|texture| all_wad_textures.contains(texture));

        if can_find_all_textures_in_listed_wads {
            wad_relative_paths
                .into_iter()
                .zip(wad_files_bytes.into_iter())
                .for_each(|(path, bytes)| {
                    resource_map.insert(path.to_owned(), bytes);
                });

            info!("Found all .wad files needed for external textures.");
        } else {
            info!(
                "Cannot find all .wad files needed for external textures. Falling back to read all .wad files."
            );

            wad_fallback_search = true;
        }
    }

    if !textures_in_external_wad.is_empty() && (!res_path.exists() || wad_fallback_search) {
        // the nuclear option where we have to scan everything
        info!("Map has external textures. Searching through all .wad files to find them.");

        let game_mods_to_check = get_game_mods_to_check(game_mod);

        let mut wad_files_required: Vec<PathBuf> = vec![];
        let mut remaining_textures: HashSet<String> =
            textures_in_external_wad.iter().cloned().collect();

        // for each game mod, add all the wads possible
        game_mods_to_check.iter().for_each(|game_mod| {
            let dir_to_read = game_dir.join(game_mod);

            info!("Checking {game_mod}");

            // early exit
            if remaining_textures.is_empty() {
                return;
            }

            let Ok(dir_reader) = std::fs::read_dir(dir_to_read.as_path()) else {
                info!(
                    "Cannot read dir `{}` to check for .wad files",
                    dir_to_read.display()
                );

                return;
            };

            let dir_entries = dir_reader
                .filter_map(|entry| entry.ok())
                .collect::<Vec<_>>();

            // for every entry, check if it is a wad file then add it to our list
            // TODO maybe short circuit this
            dir_entries.iter().for_each(|entry| {
                // early exit
                if remaining_textures.is_empty() {
                    return;
                }

                let path = entry.path();

                if !path.is_file() {
                    return;
                }

                if path.extension().is_some_and(|ext| ext == "wad") {
                    let Ok(wad) = Wad::from_file(path.as_path()) else {
                        return;
                    };

                    wad.entries.iter().for_each(|entry| {
                        // early exit
                        if remaining_textures.is_empty() {
                            return;
                        }

                        let Some(miptex) = entry.file_entry.get_mip_tex() else {
                            return;
                        };

                        let texture_name = miptex.texture_name.get_string_standard();

                        if remaining_textures.contains(&texture_name) {
                            wad_files_required.push(path.to_owned());
                            remaining_textures.remove(&texture_name);
                        }
                    });
                }
            });
        });

        if !remaining_textures.is_empty() {
            info!(
                "After searching through all game mod directories for the .wad file. It is conclusive that this map is missing textures."
            );
        } else {
            // add the wad files to our resource
            wad_files_required.iter().for_each(|path| {
                // file name is our relative path
                let file_name = path.file_name().unwrap().to_str().unwrap();

                let Ok(bytes) = std::fs::read(path) else {
                    return;
                };

                resource_map.insert(file_name.to_owned(), bytes);
            });
        }
    }

    Ok(())
}

fn get_sound(resource_map: &mut ResourceMap, bsp: &Bsp, game_dir: &Path, game_mod: &str) {
    bsp.entities.iter().for_each(|entity| {
        let is_sound_entity = entity
            .get("classname")
            .map(|classname| SOUND_ENTITIES.contains(&classname.as_str()))
            .unwrap_or(false);

        if !is_sound_entity {
            return;
        }

        // sound path doesn't include "sound" folder
        let Some(sound_path) = entity.get("message") else {
            return;
        };

        // now it contains "sound" folder
        let sound_path = format!("sound/{}", sound_path);

        if !sound_path.ends_with(".wav") {
            return;
        }

        // duplicated
        if resource_map.contains_key(&sound_path) {
            return;
        }

        let Some(sound_absolute_path) =
            search_game_resource(game_dir, game_mod, Path::new(&sound_path), true)
        else {
            warn!("Cannot find sound `{}`", sound_path);
            return;
        };

        let Ok(sound_bytes) = std::fs::read(sound_absolute_path.as_path()) else {
            warn!("Cannot load sound `{}`", sound_absolute_path.display());
            return;
        };

        resource_map.insert(sound_path, sound_bytes);
    });
}

fn get_other_sound(resource_map: &mut ResourceMap, game_dir: &Path, game_mod: &str) {
    COMMON_RESOURCE_SOUND.iter().for_each(|&sound_path| {
        let Some(sound_absolute_path) =
            search_game_resource(game_dir, game_mod, Path::new(&sound_path), true)
        else {
            warn!("Cannot find sound `{}`", sound_path);
            return;
        };

        let Ok(sound_bytes) = std::fs::read(sound_absolute_path.as_path()) else {
            warn!("Cannot load sound `{}`", sound_absolute_path.display());
            return;
        };

        resource_map.insert(sound_path.to_owned(), sound_bytes);
    });
}

fn get_viewmodel(resource_map: &mut ResourceMap, game_dir: &Path, game_mod: &str) {
    RESOURCE_VIEWMODELS.iter().for_each(|path| {
        let path = Path::new(path);

        if let Some(absolute_path) = search_game_resource(game_dir, game_mod, path, true) {
            let bytes = std::fs::read(absolute_path.as_path()).unwrap();

            resource_map.insert(path.display().to_string(), bytes);
        } else {
            warn!("Cannot find view model {}", path.display());
        };
    });
}

fn get_player_models(resource_map: &mut ResourceMap, game_dir: &Path, game_mod: &str) {
    RESOURCE_PLAYER_MODELS.iter().for_each(|path| {
        let path = Path::new(path);

        if let Some(absolute_path) = search_game_resource(game_dir, game_mod, path, true) {
            let bytes = std::fs::read(absolute_path.as_path()).unwrap();

            resource_map.insert(path.display().to_string(), bytes);
        } else {
            warn!("Cannot find player model {}", path.display());
        };
    });
}

// search through the game files by switching between different game mods just to makes sure
pub fn search_game_resource(
    game_dir: &Path,
    game_mod: &str,
    relative_path: &Path,
    case_sensitive: bool,
) -> Option<PathBuf> {
    let mut one_shot_path = game_dir.join(game_mod).join(relative_path);

    if !case_sensitive {
        case_insensitive_file_search(one_shot_path.as_path())
            // need to assign like this
            // do not exit early
            .map(|res| one_shot_path = res);
    }

    if one_shot_path.exists() {
        return one_shot_path.into();
    }

    let game_mods_to_check = get_game_mods_to_check(game_mod);

    for game_mod_to_check in game_mods_to_check {
        let mut new_path = game_dir.join(game_mod_to_check).join(relative_path);

        if !case_sensitive {
            case_insensitive_file_search(new_path.as_path())
                // need to assign like this
                // do not exit early
                .map(|res| new_path = res);
        }

        if new_path.exists() {
            return Some(new_path);
        }
    }

    None
}

// includes the original game_mod
// if game mod is unknown, we will check all of the game mods inside a provided list
fn get_game_mods_to_check(game_mod: &str) -> Vec<String> {
    let is_valve = game_mod == "valve";
    let is_download = game_mod.ends_with("downloads");
    let mut gamemods_to_check: Vec<String> = vec![game_mod.to_owned()]; // add our original game mod

    // if someone feeds in half life maps, check for valve_downloads because why not
    // otherwise, add valve to our list
    if is_valve {
        gamemods_to_check.push("valve_downloads".to_string());

        // if is valve, be done
        return gamemods_to_check;
    } else if game_mod == UNKNOWN_GAME_MOD {
        // if gmae mod is unknown then just check all of the other gmae mods just to be safe

        COMMON_GAME_MODS.iter().for_each(|&game_mod| {
            gamemods_to_check.push(game_mod.to_string());
        });
    } else {
        // we are not in "valve", we are not in "unknown"
        // so, we can just check our game mod, "_downloads", and then "valve"
        if is_download {
            let without_download = game_mod.replace("_downloads", "");

            gamemods_to_check.push(without_download);
        } else {
            gamemods_to_check.push(format!("{game_mod}_downloads"));
        }

        // then add valve
        gamemods_to_check.push("valve".to_string());
        gamemods_to_check.push("valve_downloads".to_string());
    }

    gamemods_to_check
}

// HOLY FUCKING RETARDS
fn case_insensitive_file_search(path: &Path) -> Option<PathBuf> {
    let path_parent = path.parent()?;
    let path_file_name_normalized = path.file_name()?.to_str()?.to_lowercase();

    for entry in std::fs::read_dir(path_parent).ok()? {
        let entry = entry.unwrap();
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }

        let entry_name_normalized = entry_path.file_name()?.to_str()?.to_lowercase();

        if entry_name_normalized == path_file_name_normalized {
            return Some(entry_path);
        }
    }

    None
}
