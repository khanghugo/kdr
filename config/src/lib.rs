use std::{
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
};

use ghost::GhostBlobType;
use serde::{Deserialize, Serialize};

/// For native
pub struct KDRConfig {
    pub game_dir: PathBuf,
}

/// For API server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KDRApiServerConfig {
    pub game_dir: PathBuf,

    pub common_resource: Vec<PathBuf>,

    pub replay_folders: Vec<PathBuf>,
    pub replay_formats: Vec<String>,
    pub replay_folders_search_recursively: bool,
    pub replay_unknown_format_override: Option<GhostBlobType>,

    pub port: u16,
    pub use_resmake_zip: bool,

    pub secret: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IOError: {source}")]
    IOError {
        #[source]
        source: std::io::Error,
    },

    #[error("Config parsing error: {source}")]
    TomlError {
        #[source]
        source: toml::de::Error,
    },
}

impl KDRApiServerConfig {
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        let config: KDRApiServerConfig =
            toml::from_str(s).map_err(|op| ConfigError::TomlError { source: op })?;

        Ok(config)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(path.as_ref())
            .map_err(|op| ConfigError::IOError { source: op })?;

        let mut s = String::new();

        file.read_to_string(&mut s)
            .map_err(|op| ConfigError::IOError { source: op })?;

        return Self::from_str(&s);
    }
}

#[cfg(test)]
mod test {
    use std::path::{Path, PathBuf};

    use crate::KDRApiServerConfig;

    #[test]
    fn server_config_write() {
        let config = KDRApiServerConfig {
            game_dir: PathBuf::from("/path/to/what"),
            common_resource: vec![],
            replay_folders: vec!["/path/to/foldre1".into(), "/path/to/pardre2".into()],
            replay_formats: vec!["dem".to_string(), "dat".to_string()],
            replay_folders_search_recursively: false,
            replay_unknown_format_override: Some(ghost::GhostBlobType::SRHLKZ),
            port: 3001,
            use_resmake_zip: false,
            secret: "abcd".into(),
        };

        let _s = toml::to_string(&config).unwrap();
        println!("{}", _s);
    }

    #[test]
    fn server_config_parse() {
        let config = "\
game_dir = \"/path/to/hehe\"
common_resource = [\
        \"/path/to/resource1\",
        \"/path/to/resource2\",
]
port = 3001
use_resmake_zip = true
";

        let config = KDRApiServerConfig::from_str(config);

        assert!(config.is_ok());

        let config = config.unwrap();

        assert_eq!(&config.game_dir, Path::new("/path/to/hehe"));
        assert_eq!(config.common_resource.len(), 2);
    }
}
