mod app;
mod renderer;
mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    let what = Some("/home/khang/bxt/game_isolated/".to_string());
    app::run_kdr(what);
}

#[cfg(target_arch = "wasm32")]
pub fn main() {}
