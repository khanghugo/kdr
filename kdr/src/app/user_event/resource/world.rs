use std::io::Cursor;

use kira::sound::static_sound::StaticSoundData;
use loader::{
    MapIdentifier, MapList, ProgressResourceProvider, Resource, ResourceMap, ResourceProvider,
    error::ResourceProviderError,
};
use tracing::{info, warn};

use crate::{
    app::{App, AppError, AppEvent, state::file::SelectedFileType},
    renderer::{skybox::SkyboxLoader, world_buffer::WorldLoader},
    utils::spawn_async,
};

impl App {
    pub(in crate::app::user_event) fn request_common_resource(&mut self) {
        info!("Requesting common resource");

        #[cfg(target_arch = "wasm32")]
        let Some(resource_provider) = &self.web_resource_provider else {
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        let Some(resource_provider) = &self.native_resource_provider else {
            return;
        };

        let resource_provider = resource_provider.to_owned();
        let event_loop_proxy = self.event_loop_proxy.clone();

        spawn_async(async move {
            let common_resource_future = resource_provider.request_common_resource();

            match common_resource_future.await {
                Ok(common_res) => event_loop_proxy
                    .send_event(AppEvent::ReceiveCommonResource(common_res))
                    .unwrap_or_else(|_| warn!("Cannot send ReceivedCommonResource")),
                Err(err) => {
                    warn!("Failed to get common resource");

                    event_loop_proxy
                        .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                            source: err,
                        }))
                        .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"))
                }
            }
        });
    }

    pub(in crate::app::user_event) fn receive_common_resource(
        &mut self,
        common_resource: ResourceMap,
    ) {
        info!("Received common resource");

        if common_resource.is_empty() {
            info!("Common resource data is empty");
        }

        // inserting audio from common resource
        common_resource.iter().for_each(|(k, v)| {
            if !k.ends_with(".wav") {
                return;
            }

            let cursor = Cursor::new(
                // HOLY
                v.to_owned(),
            );

            let Ok(sound_data) = StaticSoundData::from_cursor(cursor) else {
                warn!("Failed to parse audio file: `{}`", k);
                return;
            };

            self.state.audio_resource.insert(k.to_string(), sound_data);
        });

        // create view model buffer
        self.load_viewmodels(&common_resource);

        // create player model buffer
        self.load_player_models(&common_resource);

        self.state.other_resources.common_resource = common_resource;
    }

    pub(in crate::app::user_event) fn request_map(&mut self, map_identifier: MapIdentifier) {
        info!("Requesting map: {:?}", map_identifier);

        // Attempting to start audio whenever we request to load a map.
        // This is to guaranteed that there are some user actions taken
        // and the browser will kindly let us start audio stream.
        self.event_loop_proxy
            .send_event(AppEvent::MaybeStartAudioBackEnd)
            .unwrap_or_else(|_| warn!("Failed to send StartAudio"));

        // when we have a ghost and we wnat to load a map instead, we need to know what is being loaded
        // resource loading goes: ghost -> map
        // so, if we want to load map, that means we have to restart ghost if we play ghost previously
        // however, due to the resource loading order, we cannot just do that
        // so here, we need to know what kind of resource is being loaded to reset data correctly
        if matches!(
            self.state.file_state.selected_file_type,
            SelectedFileType::Bsp
        ) {
            self.state.playback_state.set_none();
        }

        #[cfg(target_arch = "wasm32")]
        let Some(resource_provider) = &self.web_resource_provider else {
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        let Some(resource_provider) = &self.native_resource_provider else {
            return;
        };

        // need to clone resource_provider because it is borrowed from self with &'1 lifetime
        // meanwhile, the spawn_local has 'static lifetime
        // resource_provider is just a url/path, so we are all good in cloning
        let resource_provider = resource_provider.to_owned();

        let event_loop_proxy = self.event_loop_proxy.clone();
        let event_loop_proxy2 = self.event_loop_proxy.clone();

        let send_receive_message = move |res: Result<Resource, ResourceProviderError>| match res {
            Ok(resource) => {
                event_loop_proxy
                    .send_event(AppEvent::ReceiveResource(resource))
                    .unwrap_or_else(|_| warn!("cannot send ReceiveResource"));
            }
            Err(err) => event_loop_proxy
                .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                    source: err,
                }))
                .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
        };

        let send_update_fetch_progress = move |v: f32| {
            event_loop_proxy2
                .send_event(AppEvent::UpdateFetchProgress(v))
                .unwrap_or_else(|_| warn!("Cannot send UpdateFetchProgress"));
        };

        self.state
            .file_state
            .start_spinner(&map_identifier.map_name);

        spawn_async(async move {
            let resource_res = resource_provider
                .get_map_with_progress(&map_identifier, move |progress| {
                    send_update_fetch_progress(progress);
                })
                .await;

            send_receive_message(resource_res);
        });
    }

    pub(in crate::app::user_event) fn receive_resource(&mut self, resource: Resource) {
        info!("Received resources");

        // from fetching to loading
        self.state.file_state.advance_spinner_state();

        let Some(render_context) = &self.render_context else {
            warn!("Received resources but no render context to render");
            return;
        };

        let event_loop_proxy = self.event_loop_proxy.clone();
        let send_message = move |bsp_resource, world_buffer, skybox_buffer| {
            event_loop_proxy
                .send_event(AppEvent::FinishCreateWorld(
                    bsp_resource,
                    world_buffer,
                    skybox_buffer,
                ))
                .unwrap_or_else(|_| warn!("Cannot send FinishCreateWorld"));
        };

        let device = render_context.device().clone();
        let queue = render_context.queue().clone();

        // TODO: load resources correctly
        // map assets can be loaded in another thread then we can send an event
        // however, gpu resources need to be on one thread
        // again, this spawn_async needs to lock device and queue
        // which also are needed in the render loop
        // so, the optimization is inside those functions
        // spawn_async(async move {
        let bsp_resource = resource.to_bsp_resource();

        let world_buffer = WorldLoader::load_static_world(&device, &queue, &bsp_resource);

        let skybox_buffer = SkyboxLoader::load_skybox(&device, &queue, &bsp_resource.skybox);

        send_message(bsp_resource, world_buffer, skybox_buffer);
        // });
    }

    pub(in crate::app::user_event) fn request_map_list(&mut self) {
        info!("Requesting map list");

        #[cfg(target_arch = "wasm32")]
        let Some(resource_provider) = &self.web_resource_provider else {
            warn!("Requesting map list without resource provider");
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        let Some(resource_provider) = &self.native_resource_provider else {
            warn!("Requesting map list without resource provider");
            return;
        };

        // need to clone resource_provider because it is borrowed from self with &'1 lifetime
        // meanwhile, the spawn_local has 'static lifetime
        // resource_provider is just a url/path, so we are all good in cloning
        let resource_provider = resource_provider.to_owned();

        let event_loop_proxy = self.event_loop_proxy.clone();
        let send_receive_message = move |res: Result<MapList, ResourceProviderError>| match res {
            Ok(map_list) => {
                event_loop_proxy
                    .send_event(AppEvent::ReceivedMapList(map_list))
                    .unwrap_or_else(|_| warn!("cannot send ReceivedMapList"));
            }
            Err(err) => event_loop_proxy
                .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                    source: err,
                }))
                .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
        };

        spawn_async(async move {
            let map_list = resource_provider.get_map_list().await;
            send_receive_message(map_list);
        });
    }

    pub(in crate::app::user_event) fn receive_map_list(&mut self, map_list: MapList) {
        let mod_count = map_list.len();
        let map_count: usize = map_list.values().map(|k| k.len()).sum();

        info!(
            "Received a map list of {} maps over {} game mods",
            map_count, mod_count
        );

        // sorting stuffs so it appears prettier
        // sorting keys
        let mut sorted_game_mod: Vec<_> = map_list.into_iter().collect();

        sorted_game_mod.sort_by(|a, b| a.0.cmp(&b.0));

        // sorting values
        let sorted_map_list: Vec<_> = sorted_game_mod
            .into_iter()
            .map(|(game_mod, maps)| {
                let mut sorted_maps: Vec<_> = maps.into_iter().collect();
                sorted_maps.sort();

                (game_mod, sorted_maps)
            })
            .collect();

        self.state.other_resources.map_list = sorted_map_list;
    }
}
