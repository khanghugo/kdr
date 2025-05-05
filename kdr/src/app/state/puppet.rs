use std::collections::VecDeque;

use cgmath::Deg;
use loader::MapIdentifier;
use puppeteer::{PuppetEvent, PuppetFrame, Puppeteer};
use tracing::warn;

use super::AppState;

pub struct PuppetState {
    pub puppeteer: Puppeteer,
    pub version: u32,
    pub selected_player: String,
    pub frames: VecDeque<PuppetFrame>,
    pub current_frame: usize,
}

// 5 seconds of 100fps
pub const MAX_BUFFER_LENGTH: usize = 500;

impl AppState {
    pub fn handle_puppet_event(&mut self, event: PuppetEvent) {
        match event {
            PuppetEvent::PuppetFrame(frame) => {
                let Some(puppet_state) = self.puppet_state.as_mut() else {
                    return;
                };

                if frame.frame.is_empty() {
                    return;
                }

                // storing the frame
                // need to store the frames first here so that the ui can have the player list
                // TODO store all the frames
                puppet_state.frames.clear();
                puppet_state.frames.push_back(frame.clone());

                let Some(viewinfo) = frame
                    .frame
                    .iter()
                    .find(|viewinfo| viewinfo.player.name == puppet_state.selected_player)
                else {
                    return;
                };

                self.render_state.camera.set_position(viewinfo.vieworg);
                self.render_state
                    .camera
                    .set_pitch(Deg(viewinfo.viewangles[0]));
                self.render_state
                    .camera
                    .set_yaw(Deg(viewinfo.viewangles[1]));

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
            PuppetEvent::Version(version) => {
                self.puppet_state
                    .as_mut()
                    .map(|state| state.version = version);
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
