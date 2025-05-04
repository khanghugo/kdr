//! This `lib.rs` is for exporting symbols to `wasm32`.
mod app;
mod renderer;
mod utils;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(
    resource_provider_base: String,
    websocket_url: Option<String>,
    fetch_map_list: bool,
    fetch_replay_list: bool,
) {
    use app::RunKDROptions;

    let options = RunKDROptions {
        resource_provider_base: resource_provider_base.into(),
        websocket_url,
        fetch_map_list,
        fetch_replay_list,
    };

    app::run_kdr(options);
}
