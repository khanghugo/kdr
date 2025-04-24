use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct WindowState {
    pub window: Arc<winit::window::Window>,
    pub is_fullscreen: bool,
    // for dimensions, it should store dimensions when not in full screen
    // so that exitting fullscreen will restore the old resolutions
    pub width: u32,
    pub height: u32,
}

impl WindowState {
    pub fn window(&self) -> Arc<winit::window::Window> {
        self.window.clone()
    }
}
