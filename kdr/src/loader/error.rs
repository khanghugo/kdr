use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResourceProviderError {
    #[error("Cannot find map `{bsp_name}`")]
    CannotFindBsp { bsp_name: String },

    #[error("Cannot parse map `{bsp_name}`: {source}")]
    CannotParseBsp {
        #[source]
        source: eyre::Report,
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
}
