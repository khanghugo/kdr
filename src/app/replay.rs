use cgmath::Deg;

use crate::ghost::GhostInfo;

use super::App;

/// How a replay is played.
pub enum ReplayPlaybackMode {
    /// One discrete replay frame for every tick.
    ///
    /// (current frame)
    ///
    /// Not really working right now but it is there.
    Immediate(usize),
    /// Interpolated replay frame for the current time in the app.
    ///
    /// Basically like a normal video.
    RealTime,
}

pub struct Replay {
    pub ghost: GhostInfo,
    pub playback_mode: ReplayPlaybackMode,
}

impl App {
    pub fn replay_tick(&mut self) {
        let Some(replay) = &self.ghost else { return };

        match replay.playback_mode {
            ReplayPlaybackMode::Immediate(_) => todo!("not planned for now until the recorder"),
            ReplayPlaybackMode::RealTime => {
                let Some(frame) = replay.ghost.get_frame(self.time.as_secs_f64(), None) else {
                    return;
                };

                // negative pitch
                self.render_state
                    .camera
                    .set_pitch(-Deg(frame.viewangles[0]));
                self.render_state.camera.set_yaw(Deg(frame.viewangles[1]));

                self.render_state
                    .camera
                    .set_position(frame.origin.to_array());

                // important
                self.render_state.camera.rebuild_orientation();
            }
        }
    }
}
