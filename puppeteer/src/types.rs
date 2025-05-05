use serde::{Deserialize, Serialize};

use crate::error::PuppeteerError;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[repr(C)]
pub struct PlayerInfo {
    pub name: String,
    pub steam_id: String,
}

impl Default for PlayerInfo {
    fn default() -> Self {
        Self {
            name: "arte".into(),
            steam_id: "1234".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[repr(C)]
pub struct ViewInfo {
    /// Information related to the player
    pub player: PlayerInfo,
    /// View origin
    pub vieworg: [f32; 3],
    /// View angles [PITCH, YAW, ROLL]
    pub viewangles: [f32; 3],
    /// Timer time
    pub timer_time: f32,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[repr(C)]
pub struct PuppetFrame {
    pub server_time: f32,
    /// A list of frames based on the number of player count.
    ///
    /// Index will match the player list. For example, player index 0 will have puppet frame index 0
    pub frame: Vec<ViewInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub enum PuppetEvent {
    PuppetFrame(PuppetFrame),
    ServerTime(f32),
    MapChange { game_mod: String, map_name: String },
    Version(u32),
}

impl PuppetEvent {
    pub fn encode_message_msgpack(&self) -> Result<Vec<u8>, PuppeteerError> {
        rmp_serde::to_vec(self).map_err(|op| PuppeteerError::CannotEncodeMessage {
            reason: op.to_string(),
        })
    }

    pub fn encode_message_json(&self) -> Result<String, PuppeteerError> {
        serde_json::to_string(self).map_err(|op| PuppeteerError::CannotEncodeMessage {
            reason: op.to_string(),
        })
    }
}

#[cfg(test)]
mod test {
    use crate::PuppetFrame;

    use super::{PuppetEvent, ViewInfo};

    // {"PuppetFrame":{"server_time":0.0,"frame":[{"player":{"name":"arte","steam_id":"1234"},"vieworg":[0.0,0.0,0.0],"viewangles":[0.0,0.0,0.0],"timer_time":0.0},{"player":{"name":"arte","steam_id":"1234"},"vieworg":[0.0,0.0,0.0],"viewangles":[0.0,0.0,0.0],"timer_time":0.0},{"player":{"name":"arte","steam_id":"1234"},"vieworg":[0.0,0.0,0.0],"viewangles":[0.0,0.0,0.0],"timer_time":0.0}]}}
    // {"ServerTime":0.0}
    // {"MapChange":{"game_mod":"cstrike","map_name":"de_dust2"}}
    // {"PlayerList":["this","is","it"]}
    #[test]
    fn event_encode_json() {
        let puppet_frame = PuppetEvent::PuppetFrame(PuppetFrame {
            server_time: 0.,
            frame: vec![ViewInfo::default(); 3],
        })
        .encode_message_json()
        .unwrap();

        let server_time = PuppetEvent::ServerTime(0.).encode_message_json().unwrap();

        let map_change = PuppetEvent::MapChange {
            game_mod: "cstrike".into(),
            map_name: "de_dust2".into(),
        }
        .encode_message_json()
        .unwrap();

        let version = PuppetEvent::Version(0).encode_message_json().unwrap();

        println!("{}", puppet_frame);
        println!("{}", server_time);
        println!("{}", map_change);
        println!("{}", version);
    }

    #[test]
    fn event_encode_msgpack() {
        let puppet_frame = PuppetEvent::PuppetFrame(PuppetFrame {
            server_time: 0.,
            frame: vec![ViewInfo::default(); 3],
        })
        .encode_message_msgpack()
        .unwrap();

        let server_time = PuppetEvent::ServerTime(0.)
            .encode_message_msgpack()
            .unwrap();

        let map_change = PuppetEvent::MapChange {
            game_mod: "cstrike".into(),
            map_name: "de_dust2".into(),
        }
        .encode_message_msgpack()
        .unwrap();

        let version = PuppetEvent::Version(0).encode_message_msgpack().unwrap();

        println!("{:?}", puppet_frame);
        println!("{:?}", server_time);
        println!("{:?}", map_change);
        println!("{:?}", version);
    }
}
