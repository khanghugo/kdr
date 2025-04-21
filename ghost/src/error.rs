use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GhostError {
    #[error("Unknown ghost format for file '{path}'")]
    UnknownFormat { path: PathBuf },

    #[error("Failed to parse demo file: {source}")]
    DemoParse {
        #[source]
        source: eyre::Report,
    },

    #[error("Invalid UTF-8 in ghost file: {path}")]
    Utf8Error {
        #[source]
        source: std::str::Utf8Error,
        path: PathBuf,
    },

    /// Ghost Parse is about parsing the data into [`GhostInfo`]. That means it is in the context of the `get_ghost` module.
    #[error("Failed to parse ghost data: {source}")]
    GhostParse {
        #[source]
        source: eyre::Report,
    },

    #[error("IO error: {source}")]
    IOError {
        #[source]
        source: std::io::Error,
    },
}
