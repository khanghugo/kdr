use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResourceProviderError {
    #[error("Cannot find map `{bsp_name}`")]
    CannotFindBsp { bsp_name: String },

    #[error("Cannot parse map `{bsp_name}`: {source}")]
    CannotParseBsp {
        #[source]
        source: bsp::error::BspError,
        bsp_name: String,
    },

    #[error("Cannot parse wad `{wad_name}`: {source}")]
    CannotParseWad {
        #[source]
        source: eyre::Report,
        wad_name: String,
    },

    #[error("Cannot find all skybox textures")]
    CannotFindSkyboxTextures,

    #[error("Cannot read file `{path}`: {source}")]
    IOError {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },

    // plural of IOError
    #[error("Cannot read files: {source}")]
    IOErrors {
        #[source]
        source: std::io::Error,
    },

    // platform specific error :()
    #[cfg(target_arch = "wasm32")]
    #[error("Cannot send POST map request: {source}")]
    PostError {
        #[source]
        source: reqwest::Error,
    },

    #[cfg(target_arch = "wasm32")]
    #[error("Error from response: {source}")]
    ResponseError {
        #[source]
        source: reqwest::Error,
    },

    #[cfg(target_arch = "wasm32")]
    #[error("Error from response bytes: {source}")]
    ResponseBytesError {
        #[source]
        source: reqwest::Error,
    },

    #[error("Cannot decompress the zip file: {source}")]
    ZipDecompress {
        #[source]
        source: zip::result::ZipError,
    },

    #[error("Cannot .bsp map file from archive")]
    BspFromArchive,
}
