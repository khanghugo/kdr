use std::{io::Write, path::Path};

use loader::ResourceIdentifier;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;
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
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("tracing started");
}
