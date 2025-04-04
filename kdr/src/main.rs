mod app;
mod ghost;
mod loader;
mod renderer;
mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    app::run_kdr();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {}
