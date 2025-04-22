use std::{
    io::Write,
    path::{Path, PathBuf},
};

use config::KDRApiServerConfig;
use ghost::{GhostBlob, get_ghost_blob_from_path};
use loader::{
    MapList, ReplayList, ResourceIdentifier, ResourceProvider,
    native::{NativeResourceProvider, scan_folder_for_files, search_game_resource},
};
use tracing::{Level, info, warn};
use tracing_subscriber::{FmtSubscriber, fmt::time::LocalTime};
use zip::{ZipWriter, write::SimpleFileOptions};

// from gchimp
pub struct WasmFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub fn zip_files(files: Vec<WasmFile>) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![];

    let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buf));

    let zip_options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for file in files {
        zip.start_file(file.name, zip_options).unwrap();
        zip.write_all(&file.bytes).unwrap();
    }

    zip.finish().unwrap();

    buf
}

pub fn sanitize_identifier(identifier: &ResourceIdentifier) -> Option<ResourceIdentifier> {
    let map_name_path: &Path = Path::new(identifier.map_name.as_str());
    let game_mod_path = Path::new(identifier.game_mod.as_str());

    let map_name = map_name_path.file_name()?.to_str()?.to_string();
    let game_mod = game_mod_path.file_name()?.to_str()?.to_string();

    let map_name = if map_name.ends_with(".bsp") {
        map_name
    } else {
        format!("{map_name}.bsp")
    };

    Some(ResourceIdentifier { map_name, game_mod })
}

pub fn start_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_timer(LocalTime::rfc_3339())
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("tracing started");
}

// writes a zip file and returns its bytes
// so, common resource is stored on memory in server, seems fine?
pub fn create_common_resource(game_dir: &Path, res: &[PathBuf]) -> Vec<u8> {
    // we dont know what game mod and we dont care
    const GAME_MOD: &str = "unknown";

    let mut wasm_files: Vec<WasmFile> = vec![];

    res.iter().for_each(|relative_path| {
        let Some(path) = search_game_resource(game_dir, GAME_MOD, relative_path, false) else {
            warn!("Cannot find common resource: `{}`", relative_path.display());
            return;
        };

        let bytes = std::fs::read(path).unwrap();

        let file = WasmFile {
            name: relative_path.display().to_string(),
            bytes,
        };

        wasm_files.push(file);
    });

    return zip_files(wasm_files);
}

pub async fn get_map_list(resource_provider: &NativeResourceProvider) -> MapList {
    let map_list = resource_provider.request_map_list().await.unwrap();

    info!(
        "Found ({}) maps for map list",
        map_list.iter().map(|e| e.1.len()).sum::<usize>()
    );

    map_list
}

pub async fn get_replay_list(config: &KDRApiServerConfig) -> ReplayList {
    let formats: Vec<&str> = config.replay_formats.iter().map(|s| s.as_str()).collect();
    let replay_list: ReplayList = config
        .replay_folders
        .iter()
        .filter_map(|folder| {
            scan_folder_for_files(
                folder,
                folder,
                &formats,
                config.replay_folders_search_recursively,
            )
        })
        .flatten()
        .collect();

    info!("Found ({}) demos for demo list", replay_list.len());

    replay_list
}

pub fn get_replay(config: &KDRApiServerConfig, replay_name: &str) -> Option<GhostBlob> {
    config.replay_folders.iter().find_map(|folder| {
        let path = folder.join(replay_name);
        let canonicalized = path.canonicalize().ok()?;

        // sanitizing the path
        if !canonicalized.starts_with(folder) {
            return None;
        }

        get_ghost_blob_from_path(
            &canonicalized,
            config.replay_unknown_format_override.clone(),
        )
        .ok()
    })
}
