use common::{REQUEST_MAP_ENDPOINT, REQUEST_MAP_GAME_MOD_QUERY, REQUEST_REPLAY_ENDPOINT};
use loader::{MapIdentifier, web::parse_location_search};
use puppeteer::Puppeteer;
use tracing::{info, warn};

use crate::app::{App, AppError, AppEvent, state::playback::puppet::Puppet};

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
                let identifier = MapIdentifier {
                    map_name: map_name.to_string(),
                    game_mod: game_mod.to_string(),
                };

                info!("Received map request in query: {:?}", identifier);

                self.event_loop_proxy
                    .send_event(AppEvent::RequestMap(identifier))
                    .unwrap_or_else(|_| warn!("Failed to send RequestResource"));
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
}
