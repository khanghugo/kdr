use std::path::Path;

pub use error::GhostError;

mod error;
mod get_ghost;

pub use get_ghost::*;
use get_ghost::{
    demo::demo_ghost_parse, romanian_jumpers::romanian_jumpers_ghost_parse,
    simen::simen_ghost_parse, surf_gateway::surf_gateway_ghost_parse,
};
use serde::{Deserialize, Serialize};

// when data sent over the net, we just need to know the variant and then parse it from the client side
#[derive(Debug, Serialize, Deserialize)]
pub enum GhostBlob {
    Demo(Vec<u8>),
    Simen(String),
    SurfGateway(String),
    RomanianJumpers(String),
    Unknown,
}

// for forcing ghost type regardless of the file format
pub enum GhostBlobType {
    Demo,
    Simen,
    SurfGateway,
    RomanianJumpers,
}

impl TryFrom<&str> for GhostBlobType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value == "simen" {
            return Ok(GhostBlobType::Simen);
        } else if value == "surf_gateway" {
            return Ok(GhostBlobType::SurfGateway);
        } else if value == "romanian_jumpers" {
            return Ok(GhostBlobType::RomanianJumpers);
        } else if value == "demo" {
            return Ok(GhostBlobType::Demo);
        } else {
            return Err(format!("unknown blob type `{}`", value).leak());
        }
    }
}

pub fn get_ghost_blob(
    path: &Path,
    overridden_option: Option<GhostBlobType>,
) -> Result<GhostBlob, GhostError> {
    if path.extension().is_none() {
        return Err(GhostError::UnknownFormat {
            path: path.to_path_buf(),
        });
    }

    let file_name = path.file_name().unwrap().display().to_string();

    // overridden options
    // good for people who don't even bother to standardize their own format
    if let Some(overridden_option) = overridden_option {
        match overridden_option {
            GhostBlobType::Demo => {
                let bytes = std::fs::read(path).map_err(|op| GhostError::IOError { source: op })?;
                return Ok(GhostBlob::Demo(bytes));
            }
            GhostBlobType::Simen | GhostBlobType::SurfGateway | GhostBlobType::RomanianJumpers => {
                let string_data = std::fs::read_to_string(path)
                    .map_err(|op| GhostError::IOError { source: op })?;

                match overridden_option {
                    GhostBlobType::Demo => unreachable!(),
                    GhostBlobType::Simen => return Ok(GhostBlob::Simen(string_data)),
                    GhostBlobType::SurfGateway => return Ok(GhostBlob::SurfGateway(string_data)),
                    GhostBlobType::RomanianJumpers => {
                        return Ok(GhostBlob::RomanianJumpers(string_data));
                    }
                }
            }
        }
    }

    // now proceed to check the file name instead
    if file_name.ends_with(".dem") {
        let bytes = std::fs::read(path).map_err(|op| GhostError::IOError { source: op })?;

        return Ok(GhostBlob::Demo(bytes));
    } else {
        let string_data =
            std::fs::read_to_string(path).map_err(|op| GhostError::IOError { source: op })?;

        if file_name.ends_with(".simen.txt") {
            return Ok(GhostBlob::Simen(string_data));
        } else if file_name.ends_with(".sg.json") {
            return Ok(GhostBlob::SurfGateway(string_data));
        } else if file_name.ends_with(".rj.json") {
            return Ok(GhostBlob::RomanianJumpers(string_data));
        } else {
            return Err(GhostError::UnknownFormat {
                path: path.to_path_buf(),
            });
        }
    };
}

pub fn get_ghost_from_blob(file_name: &str, blob: GhostBlob) -> Result<GhostInfo, GhostError> {
    match blob {
        GhostBlob::Demo(demo_bytes) => {
            let demo = dem::open_demo_from_bytes(&demo_bytes)
                .map_err(|op| GhostError::DemoParse { source: op })?;

            demo_ghost_parse(file_name, &demo)
        }
        GhostBlob::Simen(s) => simen_ghost_parse(file_name, &s),
        GhostBlob::SurfGateway(s) => surf_gateway_ghost_parse(file_name, &s),
        GhostBlob::RomanianJumpers(s) => romanian_jumpers_ghost_parse(file_name, &s),
        GhostBlob::Unknown => {
            return Err(GhostError::UnknownFormat {
                path: file_name.into(),
            });
        }
    }
    .map_err(|op| GhostError::GhostParse { source: op })
}

#[macro_export]
macro_rules! err {
    ($e: ident) => {{
        use eyre::eyre;

        Err(eyre!($e))
    }};

    ($format_string: literal) => {{
        use eyre::eyre;

        Err(eyre!($format_string))
    }};

    ($($arg:tt)*) => {{
        use eyre::eyre;

        Err(eyre!($($arg)*))
    }};
}
