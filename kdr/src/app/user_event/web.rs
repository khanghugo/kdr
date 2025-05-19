use common::{
    REQUEST_MAP_ENDPOINT, REQUEST_MAP_GAME_MOD_QUERY, REQUEST_MAP_URI_QUERY,
    REQUEST_REPLAY_ENDPOINT,
};
use loader::{
    MapIdentifier,
    web::{WebResourceProvider, parse_location_search},
};
use puppeteer::Puppeteer;
use tracing::{info, warn};

use crate::{
    app::{App, AppError, AppEvent, state::playback::puppet::Puppet},
    utils::spawn_async,
};

impl App {
    pub(super) fn parse_location_search(&mut self) {
        let Some(window) = web_sys::window() else {
            warn!("Parsing location search without window");
            return;
        };

        let Ok(search) = window.location().search() else {
            warn!("Parsing loation search without search");
            return;
        };

        let queries = parse_location_search(&search);

        // prioritize replay before map
        if let Some(replay) = queries.get(REQUEST_REPLAY_ENDPOINT) {
            info!("Received replay request in query: {}", replay);

            // audio will only start when user interaction is recorded
            self.state.audio_state.able_to_start_backend = false;
            self.state.paused = true;

            self.event_loop_proxy
                .send_event(AppEvent::RequestReplay(replay.to_string()))
                .unwrap_or_else(|_| warn!("Failed to send RequestReplay"));

            return;
        }

        if let Some(map_name) = queries.get(REQUEST_MAP_ENDPOINT) {
            if let Some(game_mod) = queries.get(REQUEST_MAP_GAME_MOD_QUERY) {
                if let Some(resource_uri_id) = queries.get(REQUEST_MAP_URI_QUERY) {
                    let identifier = MapIdentifier {
                        map_name: map_name.to_string(),
                        game_mod: game_mod.to_string(),
                    };

                    // alternatively, the host can host maps from a different server
                    info!(
                        "Received map request in query directing to a different server: {:?} @ {}",
                        identifier, resource_uri_id
                    );

                    self.event_loop_proxy
                        .send_event(AppEvent::RequestMapURI(identifier, resource_uri_id.clone()))
                        .unwrap_or_else(|_| warn!("Failed to send RequestMapURI"));
                } else {
                    let identifier = MapIdentifier {
                        map_name: map_name.to_string(),
                        game_mod: game_mod.to_string(),
                    };

                    info!("Received map request in query: {:?}", identifier);

                    self.event_loop_proxy
                        .send_event(AppEvent::RequestMap(identifier))
                        .unwrap_or_else(|_| warn!("Failed to send RequestMap"));
                }
            } else {
                warn!(
                    "Request map query without game mod query `{}`",
                    REQUEST_MAP_GAME_MOD_QUERY
                );
            }
        }
    }

    pub(super) fn create_puppeteer_connection(&mut self) {
        info!("Starting WebSocket connection");

        if let Some(ws_uri) = self.options.websocket_url.as_ref() {
            let puppeteer = {
                let res = Puppeteer::start_puppeteer(&ws_uri);
                match res {
                    Ok(x) => {
                        info!("Connected to WebSocket puppeteer server");
                        Some(x)
                    }
                    Err(err) => {
                        self.event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::WebSocketConnection))
                            .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"));

                        warn!("Cannot connect to WebSocket server `{}`: {err}", ws_uri);
                        None
                    }
                }
            };

            puppeteer.map(|puppeteer| {
                let puppet = Puppet::new(puppeteer);

                self.state.playback_state.set_puppet(puppet);
                self.state.input_state.free_cam = false;
            });
        }
    }

    pub(super) fn request_map_uri(&mut self, identifier: MapIdentifier, uri: String) {
        // we don't need resource provider
        // let Some(resource_provider) = &self.web_resource_provider else {
        //     return;
        // };

        let event_loop_proxy = self.event_loop_proxy.clone();
        let event_loop_proxy2 = self.event_loop_proxy.clone();
        let send_update_fetch_progress = move |v: f32| {
            event_loop_proxy2
                .send_event(AppEvent::UpdateFetchProgress(v))
                .unwrap_or_else(|_| warn!("Cannot send UpdateFetchProgress"));
        };

        self.state.file_state.start_spinner(&identifier.map_name);

        spawn_async(async move {
            let resource_res = WebResourceProvider::request_map_with_uri_with_progress(
                &identifier,
                &uri,
                move |progress| {
                    send_update_fetch_progress(progress);
                },
            )
            .await;

            match resource_res {
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
        });
    }
}
