use cgmath::Deg;
use ghost::{GhostFrameText, GhostInfo};

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
                // dont update anyting ghost related if paused
                // texts and such will be wastefully added
                // sound is the worst because it will spam if we pause on the right frame
                if self.paused {
                    return;
                }

                let Some((frame_idx, frame)) = replay.ghost.get_frame(self.time, None) else {
                    return;
                };

                if let Some(extra) = frame.extras {
                    extra.text.into_iter().for_each(|text| {
                        // something we do so that the final text of a channel is extended a bit longer
                        let channel = text.channel;
                        const EXTRA_TIME: f32 = 1.0;

                        self.text_state
                            .entity_text
                            .iter_mut()
                            .filter(|t| t.1.channel == channel)
                            .for_each(|t| t.1.life -= EXTRA_TIME);

                        self.text_state.entity_text.push((
                            frame_idx,
                            GhostFrameText {
                                // here we do something a bit hacky by just adding new timer to the text we want
                                life: text.life + self.time + EXTRA_TIME,
                                ..text
                            },
                        ));
                    });

                    extra.sound.into_iter().for_each(|sound| {
                        let sound_path = format!("sound/{}", &sound.file_name);

                        if let Some(sound_data) = self.audio_resource.get(&sound_path) {
                            if let Some(backend) = &mut self.audio_state.backend {
                                backend.play_audio_on_track(
                                    sound_data.clone(),
                                    0,
                                    None,
                                    false,
                                    sound.volume * self.audio_state.volume,
                                );
                            }
                        }
                    });
                }

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
