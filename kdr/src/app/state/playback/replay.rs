use cgmath::Deg;
use ghost::{GhostFrameEntityText, GhostInfo};
use loader::bsp_resource::EntityModel;

use crate::app::state::{
    AppState,
    overlay::text::{MAX_SAY_TEXT, SAY_TEXT_LIFE},
};

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
    // recorded last frame in the ghost
    // with this, we can know if we missed anything
    // and then fire all the events between current frame and last frame + 1
    pub last_frame: usize,
}

impl AppState {
    pub(super) fn process_replay_tick(&mut self, replay: &mut Replay) {
        match replay.playback_mode {
            ReplayPlaybackMode::Immediate(_) => todo!("not planned for now until the recorder"),
            ReplayPlaybackMode::FrameAccurate => todo!("will be eventually, an easy task"),
            ReplayPlaybackMode::Interpolated => {
                // dont update anyting ghost related if "paused"
                // texts and such will be wastefully added
                // sound is the worst because it will spam if we pause on the right frame
                // TODO maybe make this scope also explicitly pauses the replay?
                // when the replay reaches the end, it somehow has to pause
                if self.last_time == self.time {
                    return;
                }

                let Some((frame_idx, frame)) = replay.ghost.get_frame(self.time, None) else {
                    return;
                };

                let missing_frame_count = (frame_idx - replay.last_frame).saturating_sub(1);

                // discrete data
                replay.ghost.frames[(replay.last_frame + 1).min(frame_idx)..frame_idx]
                    .iter()
                    // chain the current frame last
                    .chain(std::iter::once(&frame))
                    .enumerate()
                    .for_each(|(chain_idx, frame)| {
                        if let Some(extra) = &frame.extras {
                            // discrete data, we don't need to add them again
                            if replay.last_frame == frame_idx {
                                return;
                            }

                            extra.entity_text.iter().for_each(|text| {
                                // something we do so that the final text of a channel is extended a bit longer
                                let channel = text.channel;
                                const EXTRA_TIME: f32 = 1.5;

                                // decrease life of all previous texts in the channel
                                self.text_state
                                    .entity_text
                                    .iter_mut()
                                    .filter(|t| t.1.channel == channel)
                                    .for_each(|t| t.1.life -= EXTRA_TIME);

                                if let Some((_, prev_text)) = self
                                    .text_state
                                    .entity_text
                                    .iter_mut()
                                    .find(|(_, prev_text)| {
                                        prev_text.location == text.location
                                            && prev_text.text == text.text
                                    })
                                {
                                    // if text is the same, extend life instead of pushing new text
                                    // need to add extra time from the time we deducted
                                    prev_text.life += text.life + EXTRA_TIME;
                                } else {
                                    // if there is no previous similar text, add new text
                                    self.text_state.entity_text.push((
                                        // need to id it correctly
                                        frame_idx - missing_frame_count + chain_idx,
                                        GhostFrameEntityText {
                                            // here we do something a bit hacky by just adding new timer to the text we want
                                            life: text.life + self.time + EXTRA_TIME,
                                            ..text.clone()
                                        },
                                    ));
                                }
                            });

                            extra.sound.iter().for_each(|sound| {
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

                            extra.say_text.iter().for_each(|saytext| {
                                self.text_state
                                    .say_text
                                    .push((self.time + SAY_TEXT_LIFE, saytext.clone()));

                                while self.text_state.say_text.len() > MAX_SAY_TEXT {
                                    self.text_state.say_text.remove(0);
                                }
                            });

                            // if let Some(entity_state) = self.entity_state.as_mut() {
                            //     if let Some(weapon_change) = &extra.weapon_change {
                            //         entity_state.viewmodel_state.active_viewmodel =
                            //             weapon_change.to_string();

                            //         // set that one model active and others inactive
                            //         entity_state
                            //             .entity_dictionary
                            //             .iter_mut()
                            //             .for_each(|entity| match &mut entity.1.model {
                            //                 EntityModel::ViewModel {
                            //                     model_name, active, ..
                            //                 } => {
                            //                     if model_name.contains(
                            //                         &entity_state.viewmodel_state.active_viewmodel,
                            //                     ) {
                            //                         *active = true;
                            //                     } else {
                            //                         *active = false;
                            //                     }
                            //                 }
                            //                 _ => {}
                            //             });
                            //     }

                            //     if let Some(weapon_sequence) = &extra.weapon_sequence {
                            //         // reset time for new sequence
                            //         entity_state.viewmodel_state.time = 0.;
                            //         entity_state
                            //             .entity_dictionary
                            //             .iter_mut()
                            //             .find(|entity| match &entity.1.model {
                            //                 EntityModel::ViewModel { model_name, .. } => model_name
                            //                     .contains(
                            //                         &entity_state.viewmodel_state.active_viewmodel,
                            //                     ),
                            //                 _ => false,
                            //             })
                            //             .map(|entity| {
                            //                 entity
                            //                     .1
                            //                     .transformation
                            //                     .get_skeletal_mut()
                            //                     .current_sequence_index = *weapon_sequence as usize;
                            //             });
                            //     }
                            // }
                        }
                    });

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

                replay.last_frame = frame_idx;
            }
        }
    }
}
