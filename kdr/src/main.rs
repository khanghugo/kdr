#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod renderer;
#[cfg(not(target_arch = "wasm32"))]
mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use app::RunKDROptions;

    let options = RunKDROptions {
        resource_provider_base: Some("/WD1/half-life".to_string()),
        websocket_url: None,
        fetch_map_list: true,
        fetch_replay_list: true,
    };

    app::run_kdr(options);
}

#[cfg(target_arch = "wasm32")]
pub fn main() {}
