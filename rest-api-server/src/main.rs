//! You MUST have gchimp and then do this over your maps folder so that the server doesn't have to process much data.
//!
//! gchimp resmake -f /path/to/<"maps" folder> --wad-check --include-default
use std::path::PathBuf;

use common::CONFIG_FILE_NAME;
use config::KDRApiServerConfig;
use server::start_server;
use utils::start_tracing;

mod send_res;
mod server;
pub(crate) mod utils;

const KDR_API_CONFIG_PATH_ENV: &str = "KDR_API_CONFIG_PATH";

pub struct ServerArgs {
    config: KDRApiServerConfig,
}

fn main() -> std::io::Result<()> {
    start_tracing();

    let config_from_env = std::env::var(KDR_API_CONFIG_PATH_ENV).map(|what| PathBuf::from(what));
    let config_from_local =
        std::env::current_exe().map(|path| path.with_file_name(CONFIG_FILE_NAME));
    let Ok(config) = config_from_env.or(config_from_local) else {
        panic!("Cannot find config file `{}`", CONFIG_FILE_NAME);
    };

    let config = match KDRApiServerConfig::from_path(config.as_path()) {
        Ok(config) => config,
        Err(err) => panic!("Cannot read config file `{}`: {}", config.display(), err),
    };

    let server_args = ServerArgs { config };

    return start_server(server_args);
}
