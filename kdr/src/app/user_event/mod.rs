use std::path::Path;

use common::UNKNOWN_GAME_MOD;
use ghost::get_ghost_blob_from_bytes;
use loader::MapIdentifier;
use tracing::{info, warn};

mod render_context;
mod resource;
#[cfg(target_arch = "wasm32")]
mod web;
mod window;

use crate::app::{
    AppError,
    state::{audio::AudioBackend, file::SelectedFileType},
};

use super::{App, AppEvent, state::file::LoadingState};

impl App {
    pub(super) fn _user_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        event: AppEvent,
    ) {
        match event {
            AppEvent::CreateRenderContext(window) => {
                self.create_render_context(window);
            }
            AppEvent::FinishCreateRenderContext(render_context) => {
                self.finish_create_render_context(render_context);
            }
            AppEvent::CreateEgui => {
                self.create_egui();
            }
            AppEvent::RequestMap(map_identifier) => {
                self.request_map(map_identifier);
            }
            AppEvent::UpdateFetchProgress(progress_x) => {
                match &mut self.state.file_state.loading_state {
                    LoadingState::Fetching { progress, .. } => {
                        *progress = progress_x;
                    }
                    _ => (),
                }
            }
            AppEvent::ReceiveResource(resource) => {
                self.receive_resource(resource);
            }
            AppEvent::FinishCreateWorld(bsp_resource, world_buffer, skybox_buffer) => {
                self.finish_create_world(bsp_resource, world_buffer, skybox_buffer);
            }
            AppEvent::NewFileSelected => {
                self.state.file_state.selected_file_type = SelectedFileType::None;

                let Some(file_path) = &self.state.file_state.selected_file else {
                    warn!("New file is said to be selected but no new file found");
                    return;
                };

                let Some(file_bytes) = &self.state.file_state.selected_file_bytes else {
                    warn!("New file bytes are not loaded");
                    return;
                };

                let file_path = Path::new(file_path);

                if file_path.extension().is_some_and(|ext| ext == "bsp") {
                    info!("Received .bsp from file dialogue");

                    let possible_game_mod = file_path
                        .parent() // maps folder
                        .and_then(|path| path.parent()) // game mod
                        .and_then(|path| path.file_name())
                        .and_then(|osstr| osstr.to_str())
                        // the server needs to understand how to intepret the unknown map
                        .unwrap_or(UNKNOWN_GAME_MOD);

                    let bsp_name = file_path.file_name().unwrap().to_str().unwrap();

                    let resource_identifier = MapIdentifier {
                        map_name: bsp_name.to_string(),
                        game_mod: possible_game_mod.to_string(),
                    };

                    self.event_loop_proxy
                        .send_event(AppEvent::RequestMap(resource_identifier))
                        .unwrap_or_else(|_| {
                            warn!("Cannot send resource request message after file dialogue")
                        });

                    self.state.file_state.selected_file_type = SelectedFileType::Bsp;
                } else {
                    let Ok(ghost_blob) = get_ghost_blob_from_bytes(
                        file_path.display().to_string().as_str(),
                        file_bytes.to_vec(),
                        None,
                    ) else {
                        // if format is not known, take us to the modal to select the format
                        self.event_loop_proxy
                            .send_event(AppEvent::UnknownFormatModal)
                            .unwrap_or_else(|_| warn!("Failed to send UnknownFormatModal"));

                        return;
                    };

                    info!("Received a replay from file dialogue");

                    self.event_loop_proxy
                        .send_event(AppEvent::ReceiveReplayBlob {
                            replay_name: file_path.to_path_buf(),
                            replay_blob: ghost_blob,
                        })
                        .unwrap_or_else(|_| warn!("Cannot send ReceiveReplayBlob"));
                }
            }
            AppEvent::RequestReplay(replay_name) => {
                self.request_replay(replay_name);
            }
            AppEvent::ReceiveReplayBlob {
                replay_name,
                replay_blob,
            } => {
                self.receive_replay_blob(replay_name, replay_blob);
            }
            AppEvent::ReceiveReplay(identifier, ghost) => {
                self.receive_replay(identifier, ghost);
            }
            AppEvent::ReceivePostProcessingUpdate(state) => {
                let Some(render_context) = &self.render_context else {
                    warn!("Received ReceivePostProcessingUpdate but no render context available");
                    return;
                };

                let mut pp = render_context.post_processing.write().unwrap();

                pp.get_kuwahara_toggle().map(|res| *res = state.kuwahara);

                pp.get_bloom_toggle().map(|res| *res = state.bloom);

                pp.get_chromatic_aberration_toggle()
                    .map(|res| *res = state.chromatic_aberration);

                pp.get_grayscale_toggle().map(|res| *res = state.grayscale);

                pp.get_posterize_toggle().map(|res| *res = state.posterize);
            }
            AppEvent::MaybeStartAudioBackEnd => {
                // We can only start audio manager after the user interacts with the site.
                if self.state.audio_state.backend.is_some() {
                    return;
                }

                // FIXME: for some reasons, on native, audio backend automagically starts even though i explicitly
                // program that it can only start with user interaction
                // it is not a bad thing, but this seems inconsistent
                if !self.state.audio_state.able_to_start_backend {
                    return;
                }

                info!("Starting audio manager");

                match AudioBackend::start() {
                    Ok(backend) => {
                        self.state.audio_state.backend = backend.into();
                    }
                    Err(err) => {
                        self.event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::AudioError { source: err }))
                            .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"));
                    }
                }
            }
            AppEvent::RequestCommonResource => {
                self.request_common_resource();
            }
            AppEvent::ReceiveCommonResource(common_resource) => {
                self.receive_common_resource(common_resource);
            }
            AppEvent::RequestMapList => {
                self.request_map_list();
            }
            AppEvent::ReceivedMapList(map_list) => {
                self.receive_map_list(map_list);
            }
            AppEvent::RequestReplayList => {
                self.request_replay_list();
            }
            AppEvent::ReceiveReplayList(replay_list) => {
                self.receive_replay_list(replay_list);
            }
            #[cfg(target_arch = "wasm32")]
            AppEvent::ParseLocationSearch => {
                self.parse_location_search();
            }
            AppEvent::UnknownFormatModal => {
                self.state.ui_state.unknown_format_modal.enabled = true;
            }
            AppEvent::RequestResize => {
                self.request_resize();
            }
            AppEvent::RequestEnterFullScreen => {
                self.request_enter_fullscreen();
            }
            AppEvent::RequestExitFullScreen => {
                self.request_exit_fullscreen();
            }
            AppEvent::RequestToggleFullScreen => {
                self.request_toggle_fullscreen();
            }
            AppEvent::CreatePuppeteerConnection => {
                self.create_puppeteer_connection();
            }
            AppEvent::ErrorEvent(app_error) => {
                warn!("Error: {}", app_error.to_string());

                // if there is error, just stop with things
                // stop the loading
                self.state.file_state.loading_state = LoadingState::Idle;

                // stop with playback
                self.state.playback_state.set_none();

                // drop all the files
                self.state.file_state.selected_file = None;
                self.state.file_state.selected_file_bytes = None;

                // toast the error
                self.state.ui_state.toaster.warning(app_error.to_string());
            }
        }
    }
}
