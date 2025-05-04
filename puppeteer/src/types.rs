use serde::{Deserialize, Serialize};

use crate::error::PuppeteerError;

#[derive(Debug, Serialize, Deserialize)]
pub struct PuppetFrame {
    /// View origin
    pub vieworg: [f32; 3],
    /// View angles [PITCH, YAW, ROLL]
    pub viewangles: [f32; 3],
    /// Frame time
    pub server_time: f32,
    /// Timer time
    pub timer_time: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PuppetEvent {
    PuppetFrame(PuppetFrame),
    ServerTime(f32),
    MapChange { game_mod: String, map_name: String },
    PlayerList(Vec<String>),
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
    use super::{PuppetEvent, PuppetFrame};

    // {"PuppetFrame":{"vieworg":[0.0,0.0,0.0],"viewangles":[0.0,0.0,0.0],"server_time":0.0,"timer_time":0.0}}
    // {"ServerTime":0.0}
    // {"MapChange":{"game_mod":"cstrike","map_name":"de_dust2"}}
    // {"PlayerList":["this","is","it"]}
    #[test]
    fn event_encode() {
        let puppet_frame = PuppetEvent::PuppetFrame(PuppetFrame {
            vieworg: [0f32; 3],
            viewangles: [0f32; 3],
            server_time: 0.,
            timer_time: 0.,
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

        let player_list = PuppetEvent::PlayerList(vec!["this".into(), "is".into(), "it".into()])
            .encode_message_json()
            .unwrap();

        println!("{}", puppet_frame);
        println!("{}", server_time);
        println!("{}", map_change);
        println!("{}", player_list);
    }
}
