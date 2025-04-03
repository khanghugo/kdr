//! This `lib.rs` is for exporting symbols to `wasm32`.
mod app;
mod ghost;
mod loader;
mod renderer;
mod utils;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
fn browser_console_log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    app::run_kdr();
}
