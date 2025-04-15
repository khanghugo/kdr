use std::{pin::Pin, sync::Arc};

// need to do like this
use super::{Duration, Instant};

use audio::AudioState;
use egui::ahash::{HashMap, HashMapExt};
use futures::FutureExt;
use input::InputState;
use kira::sound::static_sound::StaticSoundData;
use loader::ResourceMap;
use overlay::{UIState, text::TextState};
use replay::Replay;
use rfd::AsyncFileDialog;
use tracing::warn;
use winit::{event_loop::EventLoopProxy, window::Window};

use crate::renderer::RenderState;

use super::CustomEvent;

pub mod audio;
pub mod input;
pub mod movement;
pub mod overlay;
pub mod replay;

/// This is just to simply tell the program what kind of thing is being loaded so it can reasonably resetting stuffs
pub enum InputFileType {
    Bsp,
    Replay,
    None,
}

pub type SortedMapList = Vec<(String, Vec<String>)>;

#[derive(Default)]
pub struct OtherResources {
    // from MapList type, we sort it so it becomes a vector
    pub common_resource: ResourceMap,
    pub map_list: SortedMapList,
}

// Decouples states from App so that we can impl specific stuffs that affect states without affecting App.
pub struct AppState {
    // time
    pub time: f32,
    pub last_time: Instant,
    pub frame_time: f32,
    pub paused: bool,
    pub playback_speed: f32,

    pub render_state: RenderState,

    // optional ghost because we might just want to render bsp
    pub replay: Option<Replay>,
    pub selected_file: Option<String>,
    // need ot be Option just to confirm that there is no file.
    pub selected_file_bytes: Option<Vec<u8>>,
    file_dialogue_future: Option<Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + 'static>>>,
    file_bytes_future: Option<Pin<Box<dyn Future<Output = Vec<u8>> + 'static>>>,
    pub other_resources: OtherResources,

    // other states
    pub input_state: InputState,
    pub input_file_type: InputFileType,
    text_state: TextState,
    pub audio_state: AudioState,
    pub audio_resource: HashMap<String, StaticSoundData>,
    pub ui_state: UIState,

    // talk with other modules
    event_loop_proxy: EventLoopProxy<CustomEvent>,
    pub window: Option<Arc<Window>>,
}

impl AppState {
    pub fn new(event_loop_proxy: EventLoopProxy<CustomEvent>) -> Self {
        Self {
            time: 0.0,
            last_time: Instant::now(),
            frame_time: 1.,
            playback_speed: 1.0,
            paused: false,

            render_state: Default::default(),
            replay: None,

            selected_file: None,
            selected_file_bytes: None,
            file_dialogue_future: None,
            file_bytes_future: None,
            other_resources: OtherResources::default(),

            input_state: InputState::default(),
            ui_state: UIState::default(),
            input_file_type: InputFileType::None,
            text_state: TextState::default(),
            audio_state: AudioState::default(),
            audio_resource: HashMap::new(),

            event_loop_proxy,
            window: None,
        }
    }

    /// Tick function modifies everything in the app including the rendering state.
    ///
    /// If there is any event going on every frame, it should be contained in this function.
    pub fn tick(&mut self) {
        self.delta_update();

        self.interaction_tick();
        self.replay_tick();
        self.text_tick();
        self.audio_state_tick();
    }

    fn delta_update(&mut self) {
        let now = Instant::now();
        let diff = now.duration_since(self.last_time);
        self.frame_time = diff.as_secs_f32();
        self.last_time = now;

        if !(self.paused || self.replay.is_none()) {
            self.time += diff.as_secs_f32() * self.playback_speed;
        }
    }

    pub fn trigger_file_dialogue(&mut self) {
        let future = AsyncFileDialog::new()
            .add_filter("BSP/DEM", &["bsp", "dem"])
            .pick_file();

        self.file_dialogue_future = Some(Box::pin(future))
    }

    pub fn state_poll(&mut self) {
        // only read the file name, yet to have the bytes
        if let Some(future) = &mut self.file_dialogue_future {
            if let Some(file_handle) = future.now_or_never() {
                self.selected_file = file_handle.map(|f| {
                    #[cfg(not(target_arch = "wasm32"))]
                    let result = f.path().display().to_string();

                    #[cfg(target_arch = "wasm32")]
                    let result = f.file_name();

                    self.file_bytes_future = Some(Box::pin(async move {
                        let bytes = f.read().await;
                        bytes
                    }));

                    return result;
                });

                self.file_dialogue_future = None;
            }
        }

        // now have the bytes
        if let Some(future) = &mut self.file_bytes_future {
            if let Some(file_bytes) = future.now_or_never() {
                self.selected_file_bytes = file_bytes.into();
                self.file_bytes_future = None;

                // only new file when we have the bytes
                self.event_loop_proxy
                    .send_event(CustomEvent::NewFileSelected)
                    .unwrap_or_else(|_| warn!("Cannot send NewFileSelected"));
            }
        }
    }

    // probably doesnt work welll but it is good enough for now on native
    // on web, we have to think about the canvas size instead
    pub fn window_dimensions(&self) -> Option<(u32, u32)> {
        self.window.as_ref().map(|window| {
            let x = window.inner_size();
            (x.width, x.height)
        })
    }
}
