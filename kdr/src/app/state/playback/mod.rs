use puppet::Puppet;
use replay::Replay;

use super::AppState;

pub mod puppet;
pub mod replay;

pub struct PlaybackState {
    pub playback_mode: PlaybackMode,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playback_mode: PlaybackMode::None,
        }
    }
}

impl PlaybackState {
    pub fn is_none(&self) -> bool {
        matches!(self.playback_mode, PlaybackMode::None)
    }

    pub fn set_none(&mut self) {
        self.playback_mode = PlaybackMode::None;
    }

    pub fn set_replay(&mut self, replay: Replay) {
        self.playback_mode = PlaybackMode::Replay(replay);
    }

    pub fn get_replay(&self) -> Option<&Replay> {
        if let PlaybackMode::Replay(x) = &self.playback_mode {
            Some(x)
        } else {
            None
        }
    }

    pub fn set_puppet(&mut self, puppet: Puppet) {
        self.playback_mode = PlaybackMode::Live(puppet);
    }

    pub fn get_puppet(&self) -> Option<&Puppet> {
        if let PlaybackMode::Live(x) = &self.playback_mode {
            Some(x)
        } else {
            None
        }
    }

    pub fn get_puppet_mut(&mut self) -> Option<&mut Puppet> {
        if let PlaybackMode::Live(x) = &mut self.playback_mode {
            Some(x)
        } else {
            None
        }
    }
}

pub enum PlaybackMode {
    Replay(Replay),
    // Check Puppet
    Live(Puppet),
    // Map viewer
    None,
}

impl Default for PlaybackMode {
    fn default() -> Self {
        Self::None
    }
}

impl AppState {
    pub(super) fn playback_tick(&mut self) {
        // don't override the camera if in free cam
        if self.input_state.free_cam {
            return;
        }

        if self.playback_state.is_none() {
            return;
        }

        // pain pattern
        let playback_mode =
            std::mem::replace(&mut self.playback_state.playback_mode, PlaybackMode::None);

        let playback_mode = match playback_mode {
            PlaybackMode::Replay(mut replay) => {
                self.process_replay_tick(&mut replay);

                PlaybackMode::Replay(replay)
            }
            PlaybackMode::Live(mut puppet) => {
                self.process_puppet_tick(&mut puppet);

                PlaybackMode::Live(puppet)
            }
            PlaybackMode::None => PlaybackMode::None,
        };

        self.playback_state.playback_mode = playback_mode;
    }
}
