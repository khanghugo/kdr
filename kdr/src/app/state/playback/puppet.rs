use std::collections::VecDeque;

use cgmath::Deg;
use common::{lerp_arr3, lerp_viewangles};
use loader::MapIdentifier;
use puppeteer::{PuppetEvent, PuppetFrame, Puppeteer};
use tracing::warn;

use super::AppState;

pub struct Puppet {
    pub puppeteer: Puppeteer,
    pub version: u32,
    pub selected_player: String,
    pub frames: PuppetFrames,
    pub current_frame: usize,
    // // offset = server time - client time
    // // with this, client can find live time by looking at this offset
    // pub server_time_offset: f32,
}

const NO_PLAYER_SELECTED: &str = "NoPlayerSelected";

impl Puppet {
    pub fn new(puppeteer: Puppeteer) -> Self {
        Self {
            puppeteer,
            selected_player: NO_PLAYER_SELECTED.into(),
            version: 0,
            frames: PuppetFrames::new(),
            current_frame: 0,
        }
    }
}

pub struct JustViewInfo {
    vieworg: [f32; 3],
    viewangles: [f32; 3],
}

pub struct PuppetFrames(pub VecDeque<PuppetFrame>);

impl PuppetFrames {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    pub fn push_back(&mut self, frame: PuppetFrame) {
        self.0.push_back(frame);
    }

    pub fn pop_front(&mut self) {
        self.0.pop_front();
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    // currently no interpolation
    // should there be interpolation?
    pub fn get_viewinfo(&self, time: f32, player_name: &str) -> Option<(usize, JustViewInfo)> {
        let from_index = self
            .0
            .iter()
            .enumerate()
            .rev()
            .find_map(|(frame_idx, frame)| {
                if time >= frame.server_time {
                    Some(frame_idx)
                } else {
                    None
                }
            })
            // maybe early exit?
            .unwrap_or(0);

        // exit here if we dont have any frames
        let from_frame = self.get(from_index)?;

        let from_viewinfo = from_frame.frame.iter().find_map(|viewinfo| {
            if viewinfo.player.name == player_name {
                viewinfo.into()
            } else {
                None
            }
        })?;

        if from_index == self.len() - 1 {
            return Some((
                from_index,
                JustViewInfo {
                    vieworg: from_viewinfo.vieworg,
                    viewangles: from_viewinfo.viewangles,
                },
            ));
        }

        // to_index is guaranteed here because of the condition above
        let to_index = from_index + 1;

        let to_frame = &self.0[to_index];
        let to_viewinfo = to_frame.frame.iter().find_map(|viewinfo| {
            if viewinfo.player.name == player_name {
                viewinfo.into()
            } else {
                None
            }
        })?;

        let lerp_range = to_frame.server_time - from_frame.server_time;
        let lerp_target = (time - from_frame.server_time) / lerp_range;

        let lerped_vieworg = lerp_arr3(from_viewinfo.vieworg, to_viewinfo.vieworg, lerp_target);
        let lerped_viewangles = lerp_viewangles(
            from_viewinfo.viewangles,
            to_viewinfo.viewangles,
            lerp_target,
        );

        Some((
            from_index,
            JustViewInfo {
                vieworg: lerped_vieworg,
                viewangles: lerped_viewangles,
            },
        ))
    }

    pub fn get(&self, index: usize) -> Option<&PuppetFrame> {
        self.0.get(index)
    }
}

// 5 seconds of 100fps
// this is estimated to be around 6MiB of data
pub const MAX_BUFFER_LENGTH: usize = 3000;

impl AppState {
    pub(super) fn handle_puppet_event(&mut self, event: PuppetEvent) {
        match event {
            PuppetEvent::PuppetFrame(frame) => {
                // function calling handle_puppet_event should make sure that there is puppet
                let puppet = self.playback_state.get_puppet_mut().unwrap();

                // // server time offset is the first frame that we receive
                // if puppet.server_time_offset == 0. {
                //     puppet.server_time_offset = frame.server_time;
                // }

                // storing the frame
                // need to store the frames first here so that the ui can have the player list
                puppet.frames.push_back(frame);

                if puppet.frames.len() > MAX_BUFFER_LENGTH {
                    puppet.frames.pop_front();
                }
            }
            PuppetEvent::MapChange { game_mod, map_name } => {
                self.event_loop_proxy
                    .send_event(crate::app::AppEvent::RequestMap(MapIdentifier {
                        map_name: map_name.to_string(),
                        game_mod: game_mod.to_string(),
                    }))
                    .unwrap_or_else(|_| warn!("Failed to send RequestMap"));
            }
            PuppetEvent::Version(version) => {
                let puppet = self.playback_state.get_puppet_mut().unwrap();

                puppet.version = version;
            }
        }
    }

    #[allow(unused)]
    pub fn poll_puppeteer(&mut self) {
        if let Some(puppet) = self.playback_state.get_puppet_mut() {
            let events = puppet.puppeteer.poll_events();

            let (frame_events, normal_events): (Vec<_>, _) = events
                .into_iter()
                .partition(|event| matches!(event, PuppetEvent::PuppetFrame(_)));

            // process all normal events
            normal_events.into_iter().for_each(|event| {
                self.handle_puppet_event(event);
            });

            // only process last frame
            if let Some(last_frame) = frame_events.into_iter().last() {
                self.handle_puppet_event(last_frame);
            }
        }
    }

    pub(super) fn process_puppet_tick(&mut self, puppet: &mut Puppet) {
        // SELF.TIME IS IMPLICITLY SET IN THE RANGE OF ALL PUPPET FRAMES
        // BECAUSE OF THE EGUI ELEMENT
        // WHAT THE FUCK
        // SO WE DONT ADD TIMER OFFSET HERE, THAT MEANS WE DONT NEED TIME OFFSET
        let Some((frame_idx, viewinfo)) = puppet
            .frames
            .get_viewinfo(self.time, &puppet.selected_player)
        else {
            return;
        };

        // assigning current frame so UI can fetch player list
        // there is one small problem with this and that is player list will update
        // but not the render when the playback is paused
        // the alternative is to store player list.
        // Doesn't seem worth it.
        puppet.current_frame = frame_idx;

        self.render_state.camera.set_position(viewinfo.vieworg);

        // our world has correct pitch, the game doesnt
        self.render_state
            .camera
            .set_pitch(Deg(-viewinfo.viewangles[0]));

        self.render_state
            .camera
            .set_yaw(Deg(viewinfo.viewangles[1]));

        // need this to update view
        self.render_state.camera.rebuild_orientation();
    }
}
