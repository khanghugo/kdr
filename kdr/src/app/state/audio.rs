//! Audio manager
//!
//! When the map is loaded, we will check for all looping ambient_generic and allocate spatial track for all of them. Need to store it inside the struct
//! so that it doesn't go out of scope and dropped.
//!
//! For dynamic sounds like foot step or triggered sound, we must allocate a fixed number of tracks beforehand and feed into it.
//! And for that, we must somehow track which track is un/used to correctly play on a track.
//!
//! The game does contain channel index so we can mimic that.
use std::array::from_fn;

use cgmath::{Deg, InnerSpace, Rotation3};
use egui::ahash::{HashMap, HashMapExt};
use kira::{
    AudioManager, AudioManagerSettings, Tween,
    listener::ListenerHandle,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    track::{SpatialTrackBuilder, SpatialTrackHandle, TrackBuilder, TrackHandle},
};
use tracing::{info, warn};

use crate::app::AppEvent;

use super::{AppState, Duration};

const TRACK_COUNT: usize = 8;
const BASIC_TWEEN: Tween = Tween {
    start_time: kira::StartTime::Immediate,
    duration: Duration::ZERO,
    easing: kira::Easing::Linear,
};

pub enum TrackHandleType {
    NonSpatial(TrackHandle),
    Spatial(SpatialTrackHandle),
}

impl TrackHandleType {
    pub fn get_spatial_mut(&mut self) -> Option<&mut SpatialTrackHandle> {
        match self {
            Self::Spatial(x) => Some(x),
            Self::NonSpatial(_) => None,
        }
    }

    pub fn get_non_spatial_mut(&mut self) -> Option<&mut TrackHandle> {
        match self {
            Self::Spatial(_) => None,
            Self::NonSpatial(x) => Some(x),
        }
    }

    pub fn play(
        &mut self,
        sound_data: StaticSoundData,
    ) -> Result<StaticSoundHandle, kira::PlaySoundError<()>> {
        match self {
            TrackHandleType::NonSpatial(track_handle) => track_handle.play(sound_data),
            TrackHandleType::Spatial(spatial_track_handle) => spatial_track_handle.play(sound_data),
        }
    }
}

pub struct DynamicTrack {
    // use spatial track because it is nicer
    pub handle: TrackHandleType,
    // if there is current sound, it means track is not free
    pub current_sound: Option<StaticSoundHandle>,
}

impl DynamicTrack {
    pub fn is_free(&self) -> bool {
        self.current_sound.is_none()
    }
}

pub struct AmbientTrack {
    pub handle: SpatialTrackHandle,
    pub current_sound: StaticSoundHandle,
}

pub struct AudioState {
    pub backend: Option<AudioBackend>,
    /// This happens on the web where user interaction is needed for audio context to start
    pub able_to_start_backend: bool,
    pub volume: f32,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            backend: None,
            // default to not being able to start audio, even on native, :()
            able_to_start_backend: false,
            volume: 0.5,
        }
    }
}

pub struct AudioBackend {
    pub audio_manager: AudioManager,
    pub listener: ListenerHandle,
    // Key is track name
    // We are sure that these tracks are always occupied so this is ok.
    // Make sure we only choose looping ambient_generic
    // Looping ambient_generic doesn't have any flags beside radius flags
    pub ambient_tracks: HashMap<String, AmbientTrack>,
    pub spatial_dynamic_tracks: [DynamicTrack; TRACK_COUNT],
    // we actually dont need that many non spatial dynamic tracks,
    pub dynamic_tracks: [DynamicTrack; TRACK_COUNT],
}

#[derive(Debug, thiserror::Error)]
pub enum AudioStateError {
    #[error("Failed to start audio manager: {source}")]
    FailedToStartAudioManager {
        #[source]
        source: kira::backend::cpal::Error,
    },

    #[error("Failed to create listener: {source}")]
    FailedToCreateListener {
        #[source]
        source: kira::ResourceLimitReached,
    },

    #[error("Failed to create spatial track: {source}")]
    FailedToCreateSpatialTrack {
        #[source]
        source: kira::ResourceLimitReached,
    },
}

impl AudioBackend {
    pub fn start() -> Result<Self, AudioStateError> {
        let mut audio_manager = AudioManager::new(AudioManagerSettings::default())
            .map_err(|op| AudioStateError::FailedToStartAudioManager { source: op })?;

        let listener = audio_manager
            .add_listener(
                [0f32; 3],
                // great library
                mint::Quaternion {
                    v: mint::Vector3 {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    },
                    // don't start the value with 0
                    s: 1.,
                },
            )
            .map_err(|op| AudioStateError::FailedToCreateListener { source: op })?;

        let start_pos = [0f32; 3];

        let spatial_dynamic_tracks: [DynamicTrack; TRACK_COUNT] = from_fn(|_| {
            let spatial_track = audio_manager
                .add_spatial_sub_track(
                    &listener,
                    start_pos,
                    SpatialTrackBuilder::new().distances([1., 512.]),
                )
                .map_err(|op| AudioStateError::FailedToCreateSpatialTrack { source: op })
                .unwrap();

            DynamicTrack {
                handle: TrackHandleType::Spatial(spatial_track),
                current_sound: None,
            }
        });

        let dynamic_tracks: [DynamicTrack; TRACK_COUNT] = from_fn(|_| {
            let track = audio_manager
                .add_sub_track(TrackBuilder::default())
                .map_err(|op| AudioStateError::FailedToCreateSpatialTrack { source: op })
                .unwrap();

            DynamicTrack {
                handle: TrackHandleType::NonSpatial(track),
                current_sound: None,
            }
        });

        let ambient_tracks = HashMap::new();

        Ok(Self {
            audio_manager,
            listener,
            spatial_dynamic_tracks,
            ambient_tracks,
            dynamic_tracks,
        })
    }

    pub fn get_track(&mut self, track: usize, is_spatial: bool) -> &mut DynamicTrack {
        let my_my = if is_spatial {
            &mut self.spatial_dynamic_tracks
        } else {
            &mut self.dynamic_tracks
        };

        return my_my
            .get_mut(track)
            .unwrap_or_else(|| panic!("Track {} is not allocated", track));
    }

    pub fn play_audio_on_track(
        &mut self,
        audio_data: StaticSoundData,
        track: usize,
        pos: Option<[f32; 3]>,
        loop_: bool,
        volume: f32,
    ) {
        // dont play sound if volume is this low
        if volume <= 0.01 {
            return;
        }

        let is_spatial = pos.is_some();

        let track = self.get_track(track, is_spatial);
        if let Some(pos) = pos {
            if let TrackHandleType::Spatial(track) = &mut track.handle {
                track.set_position(pos, BASIC_TWEEN);
            }
        }

        // stop current sound becuase we are playing on this track
        if let Some(current_sound) = &mut track.current_sound {
            current_sound.stop(BASIC_TWEEN);
        }

        let Ok(mut audio_handle) = track.handle.play(audio_data) else {
            warn!("Cannot play audio");
            return;
        };

        // amplitude to loudness
        audio_handle.set_volume(20. * volume.log10(), BASIC_TWEEN);

        if loop_ {
            audio_handle.set_loop_region(..);
        } else {
            audio_handle.set_loop_region(None);
        }

        track.current_sound = audio_handle.into();
    }

    pub fn reset_audio(&mut self) {
        self.ambient_tracks.clear();
        self.spatial_dynamic_tracks
            .iter_mut()
            .for_each(|track| track.current_sound = None);
    }
}

impl AppState {
    pub(super) fn audio_state_tick(&mut self) {
        let Some(backend) = &mut self.audio_state.backend else {
            return;
        };

        // updating listener so we have correct spatial audio
        // TODO: make this better with just pure quaternion, righw now it is just camera code
        let pos = self.render_state.camera.pos;

        let yaw = self.render_state.camera.yaw();
        let pitch = self.render_state.camera.pitch();
        let yaw_quat =
            cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_y(), yaw - Deg(90.));

        let forward = yaw_quat * (-cgmath::Vector3::unit_z());
        let right = forward.cross(cgmath::Vector3::unit_y()).normalize();

        let pitch_quat = cgmath::Quaternion::from_axis_angle(right, pitch);

        // update orientation
        let kira_quat = pitch_quat * yaw_quat;

        let kira_quat_v = [kira_quat.v.x, kira_quat.v.y, kira_quat.v.z];

        backend
            .listener
            .set_position([pos.x, pos.y, pos.z], BASIC_TWEEN);

        backend.listener.set_orientation(
            mint::Quaternion {
                v: kira_quat_v.into(),
                s: kira_quat.s,
            },
            BASIC_TWEEN,
        );

        // update the states of channels
        backend.spatial_dynamic_tracks.iter_mut().for_each(|track| {
            if let Some(ref mut sound) = track.current_sound {
                if matches!(sound.state(), kira::sound::PlaybackState::Stopped) {
                    track.current_sound = None;
                }
            }
        });
    }

    pub fn maybe_start_audio_based_on_user_interaction(&mut self) {
        if !self.audio_state.able_to_start_backend {
            self.audio_state.able_to_start_backend = true;
            self.event_loop_proxy
                .send_event(AppEvent::MaybeStartAudioBackEnd)
                .unwrap_or_else(|_| warn!("Failed to send MaybeStartAudioBackEnd"));
        }
    }
}
