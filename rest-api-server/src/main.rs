//! You MUST have gchimp and then do this over your maps folder so that the server doesn't have to process much data.
//!
//! gchimp resmake -f /path/to/<"maps" folder> --wad-check --include-default
use std::path::PathBuf;

use common::CONFIG_FILE_NAME;
use config::KDRApiServerConfig;
use loader::native::NativeResourceProvider;
use server::start_server;
use tracing::info;
use utils::{create_common_resource, start_tracing};

mod send_res;
mod server;
pub(crate) mod utils;

const KDR_API_CONFIG_PATH_ENV: &str = "KDR_API_CONFIG_PATH";

pub struct ServerArgs {
    resource_provider: NativeResourceProvider,
    port: u16,
    common_resource: Option<Vec<u8>>,
    use_resmake_zip: bool,
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

    let game_dir = config.game_dir;
    let port = config.port;
    let resource_provider = NativeResourceProvider::new(game_dir.as_path());

    let common_resource = if config.common_resource.is_empty() {
        info!("No common resource given");
        None
    } else {
        info!(
            "Found ({}) common resources given. Creating .zip for common resources",
            config.common_resource.len()
        );
        create_common_resource(game_dir.as_path(), &config.common_resource).into()
    };

    let server_args = ServerArgs {
        resource_provider,
        port,
        common_resource,
        use_resmake_zip: config.use_resmake_zip,
    };

    return start_server(server_args);
}
