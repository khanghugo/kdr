//! You MUST have gchimp and then do this over your maps folder so that the server doesn't have to process much data.
//!
//! gchimp resmake -f /path/to/<"maps" folder> --wad-check --include-default
use std::{path::PathBuf, sync::LazyLock};

use clap::Parser;
use loader::native::NativeResourceProvider;
use server::start_server;
use utils::start_tracing;

mod send_res;
mod server;
pub(crate) mod utils;

const KDR_API_GAME_DIR_ENV: &str = "KDR_API_GAME_DIR";
const KDR_API_PORT_ENV: &str = "KDR_API_PORT";

static GAME_DIRECTORY: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var(KDR_API_GAME_DIR_ENV).ok());

static DEFAULT_PORT: u16 = 3001;

static PORT: LazyLock<Option<u16>> = LazyLock::new(|| {
    std::env::var(KDR_API_PORT_ENV)
        .ok()
        .and_then(|port_val| port_val.parse::<u16>().ok())
});

/// Parsing arguments from the """REST API SERVER""".
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct ApiServerArgs {
    /// Directory of the game, pointing to hl.exe, such as "/path/to/hl.exe"
    #[arg(short, long)]
    game_dir: Option<String>,

    /// Port the application listens on
    #[arg(short, long)]
    port: Option<u16>,

    /// Resource that is distributed when loading into the server first time. They are used between maps/demos.
    ///
    /// These type of files include foot step and hud use and whatnot.
    #[arg(short, long)]
    common_resource: Option<PathBuf>,
}

pub struct ServerArgs {
    resource_provider: NativeResourceProvider,
    port: u16,
    common_resource: Option<PathBuf>,
}

fn main() -> std::io::Result<()> {
    start_tracing();

    let args = ApiServerArgs::parse();

    let game_dir = GAME_DIRECTORY
        .clone()
        .or(args.game_dir)
        .unwrap_or_else(|| panic!("No game directory set"));

    let port = PORT.or(args.port).unwrap_or(DEFAULT_PORT);

    let resource_provider = NativeResourceProvider::new(game_dir.as_str());

    let common_resource = args.common_resource;

    let server_args = ServerArgs {
        resource_provider,
        port,
        common_resource,
    };

    return start_server(server_args);
}
