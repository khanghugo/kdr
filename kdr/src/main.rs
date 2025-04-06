mod app;
mod ghost;
pub mod loader;
mod renderer;
mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    let what = Some("/WD1/half-life".to_string());
    app::run_kdr(what);
}

#[cfg(target_arch = "wasm32")]
pub fn main() {}
