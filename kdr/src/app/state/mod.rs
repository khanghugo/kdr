use crate::renderer::camera::Camera;

// need to do like this
use super::{Duration, Instant};

use audio::AudioState;
use egui::ahash::{HashMap, HashMapExt};
use entities::EntityState;
use file::FileState;
use input::InputState;
use kira::sound::static_sound::StaticSoundData;
use loader::{ReplayList, ResourceMap};
use overlay::{UIState, text::TextState};
use playback::PlaybackState;
use render::RenderState;
use window::WindowState;
use winit::event_loop::EventLoopProxy;

use super::AppEvent;

pub mod audio;
pub mod entities;
pub mod file;
pub mod input;
mod misc;
pub mod movement;
pub mod overlay;
pub mod playback;
pub mod render;
pub mod window;

pub type SortedMapList = Vec<(String, Vec<String>)>;

#[derive(Debug, Default)]
pub struct OtherResources {
    pub bsp: Option<bsp::Bsp>,
    // from MapList type, we sort it so it becomes a vector
    pub common_resource: ResourceMap,
    pub map_list: SortedMapList,
    pub replay_list: ReplayList,
}

// Decouples states from App so that we can impl specific stuffs that affect states without affecting App.
pub struct AppState {
    /// Client time
    ///
    /// During replay playback, this syncs with demo time
    ///
    /// During live playback, this shouldn't be changed too significantly
    pub time: f32,
    pub last_time: f32,
    pub last_instant: Instant,
    pub frame_time: f32,
    pub paused: bool,
    pub playback_speed: f32,

    pub render_state: RenderState,

    pub other_resources: OtherResources,

    // other states
    pub input_state: InputState,
    pub text_state: TextState,
    pub audio_state: AudioState,
    pub audio_resource: HashMap<String, StaticSoundData>,
    pub ui_state: UIState,
    pub file_state: FileState,
    pub window_state: Option<WindowState>,
    pub playback_state: PlaybackState,
    pub entity_state: Option<EntityState>,

    // talk with other modules
    event_loop_proxy: EventLoopProxy<AppEvent>,
}

impl AppState {
    pub fn new(event_loop_proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            time: 0.,
            last_time: 0.,
            last_instant: Instant::now(),
            frame_time: 1.,
            playback_speed: 1.,
            paused: false,

            render_state: Default::default(),

            other_resources: OtherResources::default(),

            input_state: InputState::default(),
            ui_state: UIState::default(),
            text_state: TextState::default(),
            audio_state: AudioState::default(),
            audio_resource: HashMap::new(),
            file_state: FileState::default(),
            playback_state: PlaybackState::default(),
            entity_state: None,

            event_loop_proxy,
            window_state: None,
        }
    }

    /// Tick function modifies everything in the app including the rendering state.
    ///
    /// If there is any event going on every frame, it should be contained in this function.
    pub fn tick(&mut self) {
        self.delta_update();

        // polls conditionally with target_arch so that it doesn't run on native
        // polling before playback, maybe it makes sense
        #[cfg(target_arch = "wasm32")]
        self.poll_puppeteer();

        // need interaction tick to process first so that the viewmodel is updated
        // and then replay tick to get the latest view
        // TODO maybe viewmodel a different state?
        self.interaction_tick();
        self.playback_tick();
        self.entity_tick();

        self.text_tick();
        self.audio_state_tick();

        // whatever i want here
        self.misc_tick();
    }

    fn delta_update(&mut self) {
        let now = Instant::now();
        let diff = now.duration_since(self.last_instant);
        self.frame_time = diff.as_secs_f32();
        self.last_instant = now;

        // only updates delta if
        // playback and unpaused
        // or noplayback
        if !(self.paused || self.playback_state.is_none()) || self.playback_state.is_none() {
            self.last_time = self.time;
            self.time += diff.as_secs_f32() * self.playback_speed;
        }
    }

    // now this is just confusing
    // if things scale wrong or don't use absolute positions on screen
    // use this instead
    pub fn winit_window_dimensions(&self) -> Option<(u32, u32)> {
        self.window_state.as_ref().map(|window_state| {
            // need to use logical size for this
            let x = window_state
                .window()
                .inner_size()
                .to_logical(window_state.window().scale_factor());
            (x.width, x.height)
        })
    }

    // this returns the actual windows dimensions
    pub fn egui_window_dimensions(&self, egui_ctx: &egui::Context) -> Option<(u32, u32)> {
        self.window_state.as_ref().map(|window_state| {
            // need to use logical size for this
            let x = window_state
                .window()
                .inner_size()
                .to_logical(egui_ctx.pixels_per_point() as f64);
            (x.width, x.height)
        })
    }

    /// This needs camera.fovx assigned before calling
    pub(super) fn update_fov(&mut self) {
        let Some(window_state) = self.window_state.as_ref() else {
            return;
        };

        let width = window_state.width;
        let height = window_state.height;

        self.render_state.camera.fovy =
            Camera::calculate_y_fov(self.render_state.camera.fovx, width as f32, height as f32);

        // only change the aspects
        self.render_state.camera.aspect = width as f32 / height as f32;
    }
}
