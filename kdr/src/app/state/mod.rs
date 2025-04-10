use std::pin::Pin;

// need to do like this
use super::{Duration, Instant};

use movement::Key;
use replay::Replay;
use winit::event_loop::EventLoopProxy;

use crate::renderer::RenderState;

use super::CustomEvent;

pub mod movement;
pub mod overlay;
pub mod replay;

// Decouples states from App so that we can impl specific stuffs that affect states without affecting App.
pub struct AppState {
    // time
    pub time: Duration,
    pub last_time: Instant,
    pub frame_time: f32,
    pub paused: bool,

    // stuffs
    // TODO future render state might need to be optional so that we can reload map or something?? not sure
    // like we can start the app with nothing going on and hten drag and rdop the map 🤤
    pub render_state: RenderState,
    // optional ghost because we might just want to render bsp
    pub ghost: Option<Replay>,
    pub selected_file: Option<String>,
    // need ot be Option just to confirm that there is no file.
    pub selected_file_bytes: Option<Vec<u8>>,
    file_dialogue_future: Option<Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + 'static>>>,
    file_bytes_future: Option<Pin<Box<dyn Future<Output = Vec<u8>> + 'static>>>,

    // input
    keys: Key,
    mouse_right_hold: bool,

    event_loop_proxy: EventLoopProxy<CustomEvent>,
}

impl AppState {
    pub fn new(event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
        Self {
            time: Duration::ZERO,
            last_time: Instant::now(),
            frame_time: 1.,
            render_state: Default::default(),
            keys: Key::empty(),
            mouse_right_hold: false,
            ghost: None,
            selected_file: None,
            selected_file_bytes: None,
            file_dialogue_future: None,
            file_bytes_future: None,
            paused: false,
            event_loop_proxy,
        }
    }

    /// Tick function modifies everything in the app including the rendering state.
    ///
    /// If there is any event going on every frame, it should be contained in this function.
    pub fn tick(&mut self) {
        self.delta_update();

        self.interaction_tick();
        self.replay_tick();
    }

    fn delta_update(&mut self) {
        let now = Instant::now();
        let diff = now.duration_since(self.last_time);
        self.frame_time = diff.as_secs_f32();
        self.last_time = now;
        self.time += diff;
    }
}
