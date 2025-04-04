use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    str::from_utf8,
};

use dem::{open_demo_from_bytes, types::Demo};
pub use error::GhostError;

mod error;
mod get_ghost;

pub use get_ghost::GhostInfo;

pub enum GhostBlob<'a> {
    Demo(Demo),
    Simen(&'a str),
    SurfGateway(&'a str),
    RomanianJumper(&'a str),
    Unknown,
}

/// In web browser term, the path is a fake path.
///
/// The client needs to run this command at most 2 times just to verify that we have a correct ghost blob.
fn categorise_ghost_blob<'a>(
    path: impl AsRef<Path> + AsRef<OsStr>,
    ghost_blob: &'a [u8],
) -> Result<GhostBlob<'a>, GhostError> {
    let filename: &Path = path.as_ref();
    let filename = filename.file_name().unwrap().to_str().unwrap();

    let path: &Path = path.as_ref();

    if filename.ends_with(".dem") {
        return open_demo_from_bytes(ghost_blob)
            .map(GhostBlob::Demo)
            .map_err(|err| GhostError::DemoParse { source: err });
    } else {
        let file = from_utf8(ghost_blob).map_err(|err| GhostError::Utf8Error {
            source: err,
            path: path.to_path_buf(),
        })?;

        if filename.ends_with(".simen.txt") {
            return Ok(GhostBlob::Simen(file));
        } else if filename.ends_with(".sg.json") {
            return Ok(GhostBlob::SurfGateway(file));
        } else if path.ends_with(".rj.json") {
            return Ok(GhostBlob::Simen(file));
        }
    }

    Err(GhostError::UnknownFormat {
        path: path.to_path_buf(),
    })
}

pub fn get_ghost<'a>(
    path: impl AsRef<Path> + AsRef<OsStr>,
    ghost_blob: &'a [u8],
) -> Result<GhostInfo, GhostError> {
    let path: &Path = path.as_ref();
    let ghost_blob = categorise_ghost_blob(path, ghost_blob)?;

    // get ghost galore
    get_ghost::get_ghost(path, ghost_blob).map_err(|err| GhostError::GhostParse { source: err })
}
