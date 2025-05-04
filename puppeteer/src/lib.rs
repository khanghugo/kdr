//! This is a web only feature where a web socket server can control an instance.
//!
//! The goal is that a kdr instance can stream live view of a server.

mod constants;
pub mod error;
mod types;

pub use constants::*;
pub use types::*;

use error::PuppeteerError;
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
};
use gloo_net::websocket::{Message, futures::WebSocket};
use tracing::warn;
use wasm_bindgen_futures::spawn_local;

pub struct Puppeteer {
    // buffered playback means we can rewind and whatever, that is for later
    // pub frame_buffer: VecDeque<PuppetFrame>,
    // client has its own timer
    // when connecting to a server, it will tell us its current time
    // with that, the client can has its own timer
    pub server_time_offset: f32,
    // sender is doing nothing, whatever
    pub event_receiver: UnboundedReceiver<PuppetEvent>,
    pub command_sender: UnboundedSender<String>,
}

impl Puppeteer {
    pub fn start_puppeteer(ws_url: &str) -> Result<Self, PuppeteerError> {
        let ws_connection =
            WebSocket::open(ws_url).map_err(|op| PuppeteerError::CannotStartWssConnection {
                reason: op.to_string(),
            })?;

        let (mut ws_sender, ws_receiver) = ws_connection.split();
        // from server to client
        let (event_sender, event_receiver) = futures::channel::mpsc::unbounded::<PuppetEvent>();
        // fromt client to server
        let (command_sender, mut command_receiver) = futures::channel::mpsc::unbounded::<String>();

        spawn_local(async move {
            // need to turn to FusedStream
            let mut ws_receiver = ws_receiver.fuse();

            let mut handle_command = async |command: String| {
                ws_sender.send(Message::Text(command)).await.unwrap();
            };

            loop {
                futures::select! {
                    msg = ws_receiver.next() => {
                        let Some(Ok(msg)) = msg else {
                            warn!("Cannot receive message");
                            break;
                        };

                        // support both json and msgpack because why not
                        let event: PuppetEvent = match msg {
                            Message::Text(json_text) => {
                                serde_json::from_str(&json_text).expect("cannot parse json")
                            }
                            Message::Bytes(items) => {
                                rmp_serde::from_slice(&items).expect("cannot parse msgpack")
                            }
                        };

                        if let Err(what) = event_sender.clone().send(event).await {
                            warn!("Error sending WS message: {what}");
                            warn!("Terminating WS connection");

                            break;
                        }
                    }
                    cmd = command_receiver.next() => {
                        let Some(cmd) = cmd else {
                            break;
                        };

                        handle_command(cmd).await;
                    }
                };
            }

            warn!("WS connection is terminated");
        });

        let res = Self {
            // frame_buffer: VecDeque::new(),
            server_time_offset: 0.,
            event_receiver,
            command_sender,
        };

        Ok(res)
    }

    // frame time accurate playback, huh
    // pub fn get_puppet_frame(&self, client_time: f32) -> Option<&PuppetFrame> {
    //     let corrected_time = client_time + self.server_time_offset;

    //     self.frame_buffer
    //         .iter()
    //         .rev()
    //         .find(|frame| frame.server_time <= corrected_time)
    // }

    // "It is not recommended to call this function from inside of a future"
    pub fn poll_event(&mut self) -> Option<PuppetEvent> {
        while let Ok(Some(event)) = self.event_receiver.try_next() {
            return event.into();
        }

        match self.event_receiver.try_next() {
            Ok(Some(event)) => event.into(),
            Ok(None) => None,
            Err(_) => None,
        }
    }

    // will receive player list in an event
    pub fn request_player_list(&mut self) -> Result<(), PuppeteerError> {
        self.command_sender
            .unbounded_send(REQUEST_PLAYER_LIST.to_string())
            .map_err(|_op| PuppeteerError::CannotSendCommand)
    }

    pub fn change_player(&mut self, player_name: &str) -> Result<(), PuppeteerError> {
        self.command_sender
            .unbounded_send(format!("{} {}", CHANGE_PLAYER, player_name))
            .map_err(|_op| PuppeteerError::CannotSendCommand)
    }
}
