use cgmath::Deg;
use loader::MapIdentifier;
use puppeteer::{PuppetEvent, Puppeteer};
use tracing::warn;

use super::AppState;

pub struct PuppetState {
    pub puppeteer: Puppeteer,
    pub player_list: Vec<String>,
    pub selected_player: usize,
}

impl AppState {
    pub fn handle_puppet_event(&mut self, event: PuppetEvent) {
        match event {
            PuppetEvent::PuppetFrame { server_time, frame } => {
                let Some(puppet_state) = self.puppet_state.as_ref() else {
                    return;
                };

                if frame.is_empty() {
                    return;
                }

                let puppet_frame = &frame[puppet_state.selected_player];

                self.render_state.camera.set_position(puppet_frame.vieworg);
                self.render_state
                    .camera
                    .set_pitch(Deg(puppet_frame.viewangles[0]));
                self.render_state
                    .camera
                    .set_yaw(Deg(puppet_frame.viewangles[1]));

                // need this to update view
                self.render_state.camera.rebuild_orientation();
            }
            PuppetEvent::ServerTime(_) => todo!(),
            PuppetEvent::MapChange { game_mod, map_name } => {
                self.event_loop_proxy
                    .send_event(crate::app::AppEvent::RequestMap(MapIdentifier {
                        map_name,
                        game_mod,
                    }))
                    .unwrap_or_else(|_| warn!("Failed to send RequestMap"));
            }
            PuppetEvent::PlayerList(items) => {
                self.puppet_state.as_mut().map(|puppet_state| {
                    puppet_state.player_list = items;
                });
            }
        }
    }

    #[allow(unused)]
    pub fn poll_puppeteer(&mut self) {
        if let Some(puppet_state) = &mut self.puppet_state {
            if let Some(event) = puppet_state.puppeteer.poll_event() {
                self.handle_puppet_event(event);
            }
        }
    }

    #[allow(unused)]
    pub fn is_puppet(&self) -> bool {
        self.puppet_state.is_some()
    }
}
