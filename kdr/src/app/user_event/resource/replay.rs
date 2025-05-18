use std::path::PathBuf;

use ghost::{GhostBlob, GhostInfo};
use loader::{MapIdentifier, ReplayList, ResourceProvider, error::ResourceProviderError};
use tracing::{info, warn};

use crate::{
    app::{
        App, AppError, AppEvent,
        state::{
            file::SelectedFileType,
            playback::replay::{Replay, ReplayPlaybackMode},
        },
    },
    utils::spawn_async,
};

impl App {
    pub(in crate::app::user_event) fn request_replay(&mut self, replay_name: String) {
        info!("Requesting replay `{}`", replay_name);

        #[cfg(not(target_arch = "wasm32"))]
        let Some(provider) = &self.native_resource_provider else {
            warn!("Cannot find native resource provider");

            self.event_loop_proxy
                .send_event(AppEvent::ErrorEvent(AppError::NoProvider))
                .unwrap_or_else(|_| warn!("Failed to send NoProvider"));

            return;
        };

        #[cfg(target_arch = "wasm32")]
        let Some(provider) = &self.web_resource_provider else {
            warn!("Cannot find web resource provider");
            // TODO send to error
            return;
        };

        let provider = provider.clone();
        let replay_name2 = replay_name.clone();

        let event_loop_proxy = self.event_loop_proxy.clone();
        let send_message =
            move |replay_request_result: Result<GhostBlob, ResourceProviderError>| {
                match replay_request_result {
                    Ok(ghost_blob) => {
                        event_loop_proxy
                            .send_event(AppEvent::ReceiveReplayBlob {
                                replay_name: replay_name.clone().into(),
                                replay_blob: ghost_blob,
                            })
                            .unwrap_or_else(|_| warn!("Failed to send ReceivedGhostRequest"));
                    }
                    Err(op) => event_loop_proxy
                        .send_event(AppEvent::ErrorEvent(AppError::ProviderError { source: op }))
                        .unwrap_or_else(|_| warn!("Failed to send ErrorEvent")),
                }
            };

        spawn_async(async move {
            let what = provider.request_replay(&replay_name2).await;

            send_message(what);
        });
    }

    pub(in crate::app::user_event) fn receive_replay_blob(
        &mut self,
        replay_name: PathBuf,
        replay_blob: GhostBlob,
    ) {
        info!("Received replay blob");

        let event_loop_proxy = self.event_loop_proxy.clone();
        let send_message = move |identifier, ghost| {
            event_loop_proxy
                .send_event(AppEvent::ReceiveReplay(identifier, ghost))
                .unwrap_or_else(|_| warn!("Failed to send ReceivedGhostRequest"));
        };

        #[cfg(not(target_arch = "wasm32"))]
        let Some(provider) = &self.native_resource_provider else {
            warn!("Cannot find native resource provider");

            self.event_loop_proxy
                .send_event(AppEvent::ErrorEvent(AppError::NoProvider))
                .unwrap_or_else(|_| warn!("Failed to send NoProvider"));

            return;
        };

        #[cfg(target_arch = "wasm32")]
        let Some(provider) = &self.web_resource_provider else {
            warn!("Cannot find web resource provider");
            // TODO send to error
            return;
        };

        let provider = provider.clone();

        spawn_async(async move {
            let Ok((identifier, ghost)) = provider.get_ghost_data(replay_name, replay_blob).await
            else {
                warn!("Cannot load ghost data");
                // TODO send error here
                return;
            };

            send_message(identifier, ghost);
        });

        self.state.file_state.selected_file_type = SelectedFileType::Replay;
    }

    pub(in crate::app::user_event) fn receive_replay(
        &mut self,
        identifier: MapIdentifier,
        ghost: GhostInfo,
    ) {
        info!("Finished processing .dem. Loading replay");

        let replay = Replay {
            ghost,
            playback_mode: ReplayPlaybackMode::Interpolated,
            last_frame: 0,
        };

        self.state.playback_state.set_replay(replay);

        // make sure the user cannot move camera because it is true by default
        self.state.input_state.free_cam = false;

        self.event_loop_proxy
            .send_event(AppEvent::RequestMap(identifier))
            .unwrap_or_else(|_| warn!("Failed to send RequestResource"));
    }

    pub(in crate::app::user_event) fn request_replay_list(&mut self) {
        info!("Requesting replay list");

        #[cfg(target_arch = "wasm32")]
        let Some(resource_provider) = &self.web_resource_provider else {
            warn!("Requesting replay list without resource provider");
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        let Some(resource_provider) = &self.native_resource_provider else {
            warn!("Requesting replay list without resource provider");
            return;
        };

        let resource_provider = resource_provider.to_owned();

        let event_loop_proxy = self.event_loop_proxy.clone();
        let send_receive_message = move |res: Result<ReplayList, ResourceProviderError>| match res {
            Ok(replay_list) => {
                event_loop_proxy
                    .send_event(AppEvent::ReceiveReplayList(replay_list))
                    .unwrap_or_else(|_| warn!("cannot send ReceiveReplayList"));
            }
            Err(err) => event_loop_proxy
                .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                    source: err,
                }))
                .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
        };

        spawn_async(async move {
            let replay_list = resource_provider.request_replay_list().await;
            send_receive_message(replay_list);
        });
    }

    pub(in crate::app::user_event) fn receive_replay_list(&mut self, replay_list: ReplayList) {
        let replay_count = replay_list.len();

        info!("Received a replay list of {} replays", replay_count);

        self.state.other_resources.replay_list = replay_list;
    }
}
