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

const DEMO_CODE: &str = "demo";
const SIMEN_CODE: &str = "simen";
const SURF_GATEWAY_CODE: &str = "surf_gateway";
const ROMANIAN_JUMPERS_CODE: &str = "romanian_jumpers";
const SOURCERUNS_HLKZ_CODE: &str = "hlkz";

impl TryFrom<&str> for GhostBlobType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value == SIMEN_CODE {
            return Ok(GhostBlobType::Simen);
        } else if value == SURF_GATEWAY_CODE {
            return Ok(GhostBlobType::SurfGateway);
        } else if value == ROMANIAN_JUMPERS_CODE {
            return Ok(GhostBlobType::RomanianJumpers);
        } else if value == DEMO_CODE {
            return Ok(GhostBlobType::Demo);
        } else if value == SOURCERUNS_HLKZ_CODE {
            return Ok(GhostBlobType::SRHLKZ);
        } else {
            return Err(format!("unknown blob type `{}`", value).leak());
        }
    }
}

impl From<GhostBlobType> for &str {
    fn from(value: GhostBlobType) -> Self {
        match value {
            GhostBlobType::Demo => DEMO_CODE,
            GhostBlobType::Simen => SIMEN_CODE,
            GhostBlobType::SurfGateway => SURF_GATEWAY_CODE,
            GhostBlobType::RomanianJumpers => ROMANIAN_JUMPERS_CODE,
            GhostBlobType::SRHLKZ => SOURCERUNS_HLKZ_CODE,
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

// we only care about ghost blob is because it will be sent from server to client to process
pub fn get_ghost_blob_from_path(
    file_name: &Path,
    overridden_option: Option<GhostBlobType>,
) -> Result<GhostBlob, GhostError> {
    let data = std::fs::read(file_name).map_err(|op| GhostError::IOError { source: op })?;

    get_ghost_blob_from_bytes(
        file_name.display().to_string().as_str(),
        data,
        overridden_option,
    )
}

pub fn get_ghost_blob_from_bytes(
    file_name: &str,
    data: Vec<u8>,
    overridden_option: Option<GhostBlobType>,
) -> Result<GhostBlob, GhostError> {
    let Some(blob_type) = overridden_option.or(GhostBlobType::try_from_file_name(file_name)) else {
        return Err(GhostError::UnknownFormat {
            path: file_name.into(),
        });
    };

    match blob_type {
        GhostBlobType::Demo => Ok(GhostBlob::Demo(data)),
        GhostBlobType::Simen | GhostBlobType::SurfGateway | GhostBlobType::RomanianJumpers => {
            let s_data = str::from_utf8(&data).map_err(|op| GhostError::Utf8Error {
                source: op.into(),
                path: file_name.into(),
            })?;

            let s_data = s_data.to_string();

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
