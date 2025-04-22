use std::path::Path;

pub use error::GhostError;

mod error;
mod get_ghost;

pub use get_ghost::*;
use get_ghost::{
    demo::demo_ghost_parse, romanian_jumpers::romanian_jumpers_ghost_parse,
    simen::simen_ghost_parse, sourceruns_hlkz::srhlkz_ghost_parse,
    surf_gateway::surf_gateway_ghost_parse,
};
use serde::{Deserialize, Serialize};

// when data sent over the net, we just need to know the variant and then parse it from the client side
#[derive(Debug, Serialize, Deserialize)]
pub enum GhostBlob {
    Demo(Vec<u8>),
    Simen(String),
    SurfGateway(String),
    RomanianJumpers(String),
    SRHLKZ(Vec<u8>),
    Unknown,
}

// for forcing ghost type regardless of the file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostBlobType {
    Demo,
    Simen,
    SurfGateway,
    RomanianJumpers,
    SRHLKZ,
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
        } else if value == "hlkz" {
            return Ok(GhostBlobType::SRHLKZ);
        } else {
            return Err(format!("unknown blob type `{}`", value).leak());
        }
    }
}

impl GhostBlobType {
    pub fn try_from_file_name(s: &str) -> Option<Self> {
        if s.ends_with(".dem") {
            Self::Demo.into()
        } else if s.ends_with(".simen.txt") {
            Self::Simen.into()
        } else if s.ends_with(".sg.json") {
            Self::SurfGateway.into()
        } else if s.ends_with(".rj.json") {
            Self::RomanianJumpers.into()
        } else if s.ends_with(".dat") {
            Self::SRHLKZ.into()
        } else {
            None
        }
    }
}

// for native to fetch data and send to some kind of processor
// also used by server to fetch blob and send through the net
pub fn get_ghost_blob_from_path(
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
            GhostBlobType::Demo | GhostBlobType::SRHLKZ => {
                let bytes = std::fs::read(path).map_err(|op| GhostError::IOError { source: op })?;

                return match overridden_option {
                    GhostBlobType::Demo => Ok(GhostBlob::Demo(bytes)),
                    GhostBlobType::SRHLKZ => Ok(GhostBlob::SRHLKZ(bytes)),
                    GhostBlobType::RomanianJumpers
                    | GhostBlobType::Simen
                    | GhostBlobType::SurfGateway => unreachable!(),
                };
            }
            GhostBlobType::Simen | GhostBlobType::SurfGateway | GhostBlobType::RomanianJumpers => {
                let string_data = std::fs::read_to_string(path)
                    .map_err(|op| GhostError::IOError { source: op })?;

                return match overridden_option {
                    GhostBlobType::Demo | GhostBlobType::SRHLKZ => unreachable!(),
                    GhostBlobType::Simen => Ok(GhostBlob::Simen(string_data)),
                    GhostBlobType::SurfGateway => Ok(GhostBlob::SurfGateway(string_data)),
                    GhostBlobType::RomanianJumpers => Ok(GhostBlob::RomanianJumpers(string_data)),
                };
            }
        }
    }

    let Some(blob_type) = GhostBlobType::try_from_file_name(&file_name) else {
        return Err(GhostError::UnknownFormat {
            path: file_name.into(),
        });
    };

    match blob_type {
        GhostBlobType::Demo | GhostBlobType::SRHLKZ => {
            let bytes = std::fs::read(path).map_err(|op| GhostError::IOError { source: op })?;

            match blob_type {
                GhostBlobType::Demo => Ok(GhostBlob::Demo(bytes)),
                GhostBlobType::Simen
                | GhostBlobType::SurfGateway
                | GhostBlobType::RomanianJumpers => unreachable!(),
                GhostBlobType::SRHLKZ => Ok(GhostBlob::SRHLKZ(bytes)),
            }
        }
        GhostBlobType::Simen | GhostBlobType::SurfGateway | GhostBlobType::RomanianJumpers => {
            let s_data =
                std::fs::read_to_string(path).map_err(|op| GhostError::IOError { source: op })?;

            match blob_type {
                GhostBlobType::Demo | GhostBlobType::SRHLKZ => unreachable!(),
                GhostBlobType::Simen => Ok(GhostBlob::Simen(s_data)),
                GhostBlobType::SurfGateway => Ok(GhostBlob::SurfGateway(s_data)),
                GhostBlobType::RomanianJumpers => Ok(GhostBlob::RomanianJumpers(s_data)),
            }
        }
    }
}

// for file dialogue where it tries to categorize the bytes
// file dialogue knows the bytes, but not whether it is ghost blob or map blob, it only knows the map name
pub fn get_ghost_blob_from_bytes(file_name: &str, data: Vec<u8>) -> Result<GhostBlob, GhostError> {
    let Some(blob_type) = GhostBlobType::try_from_file_name(file_name) else {
        return Err(GhostError::UnknownFormat {
            path: file_name.into(),
        });
    };

    match blob_type {
        GhostBlobType::Demo => Ok(GhostBlob::Demo(data)),
        GhostBlobType::Simen | GhostBlobType::SurfGateway | GhostBlobType::RomanianJumpers => {
            let s_data = String::from_utf8(data).unwrap();

            match blob_type {
                GhostBlobType::Demo | GhostBlobType::SRHLKZ => unreachable!(),
                GhostBlobType::Simen => Ok(GhostBlob::Simen(s_data)),
                GhostBlobType::SurfGateway => Ok(GhostBlob::SurfGateway(s_data)),
                GhostBlobType::RomanianJumpers => Ok(GhostBlob::RomanianJumpers(s_data)),
            }
        }
        GhostBlobType::SRHLKZ => Ok(GhostBlob::SRHLKZ(data)),
    }
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
        GhostBlob::SRHLKZ(items) => srhlkz_ghost_parse(file_name, &items),
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
