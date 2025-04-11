use cgmath::Deg;
use ghost::GhostInfo;

use super::*;

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
    /// Basically like how demo playback works.
    Interpolated,
    /// No interpolation.
    ///
    // TODO: make ghost playback do this
    FrameAccurate,
}

pub struct Replay {
    pub ghost: GhostInfo,
    pub playback_mode: ReplayPlaybackMode,
}

impl AppState {
    pub fn replay_tick(&mut self) {
        // don't override the camera if in free cam
        if self.input_state.free_cam {
            return;
        }

        let Some(replay) = &self.replay else { return };

        match replay.playback_mode {
            ReplayPlaybackMode::Immediate(_) => todo!("not planned for now until the recorder"),
            ReplayPlaybackMode::FrameAccurate => todo!("will be eventually, an easy task"),
            ReplayPlaybackMode::Interpolated => {
                let Some(frame) = replay.ghost.get_frame(self.time, None) else {
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
