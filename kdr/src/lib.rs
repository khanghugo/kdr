//! This `lib.rs` is for exporting symbols to `wasm32`.
mod app;
mod ghost;
// This is public so that the REST API process can re-use its native functions.
pub mod loader;
mod renderer;
mod utils;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    app::run_kdr("http://localhost:3001".to_string().into());
}
