//! Sends a .zip file containing all files related to a map.
//!
//! The path in the .zip file must be following:
//! .
//! ├── c4a2a.wad
//! ├── gfx
//! │   └── env
//! │       ├── neb6bk.tga
//! │       ├── neb6dn.tga
//! │       ├── neb6ft.tga
//! │       ├── neb6lf.tga
//! │       ├── neb6rt.tga
//! │       └── neb6up.tga
//! ├── maps
//! │   ├── c4a2a.bsp
//! │   └── c4a2a.res
//! ├── sound
//! │   ├── ambience
//! │   │   ├── alien_beacon.wav
//! │   │   ├── alienvoices1.wav
//! │   │   └── alienwind1.wav
//! │   └── doors
//! │       └── aliendoor3.wav
//! └── sprites
//!     └── xspark2.spr
//!
//! This means, there is no root folder and there is .wad file if needed.
//!
//! If this way isn't enough (depending on how gchimp creates zip archive after all), we can fall back to [`NativeResourceProvider`]
//! and create the zip file ourselves with the data hash map.

use std::path::PathBuf;

use kdr::loader::{
    ResourceIdentifier,
    native::{NativeResourceProvider, search_game_resource},
};

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Cannot find pre-processed zip archive containing all resources.")]
    CannotFindZip,

    #[error("Cannot read file `{path}`: {source}")]
    IOError {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },
}

pub fn gchimp_resmake(
    // identifier should already be sanitized at this point
    identifier: &ResourceIdentifier,
    resource_provider: &NativeResourceProvider,
) -> Result<Vec<u8>, ServerError> {
    let map_relative_path = PathBuf::new().join("maps").join(&identifier.map_name);
    let zip_relative_path = map_relative_path.with_extension("zip");

    let zip_file_path = search_game_resource(
        &resource_provider.game_dir,
        &identifier.game_mod,
        &zip_relative_path,
        true,
    )
    .ok_or_else(|| ServerError::CannotFindZip)?;

    let zip_bytes = std::fs::read(zip_file_path.as_path()).map_err(|op| ServerError::IOError {
        source: op,
        path: zip_file_path,
    })?;

    Ok(zip_bytes)
}
