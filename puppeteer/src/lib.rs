//! This is a web only feature where a web socket server can control an instance.
//!
//! The goal is that a kdr instance can stream live view of a server.

mod constants;
pub mod error;
mod types;

pub use types::*;

use error::PuppeteerError;
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{Receiver, UnboundedSender},
};
use gloo_net::websocket::{Message, futures::WebSocket};
use tracing::warn;
use wasm_bindgen_futures::spawn_local;

pub struct Puppeteer {
    // sender is doing nothing, whatever
    pub event_receiver: Receiver<PuppetEvent>,
    pub command_sender: UnboundedSender<String>,
}

impl Puppeteer {
    pub fn start_puppeteer(ws_url: &str) -> Result<Self, PuppeteerError> {
        let ws_connection =
            WebSocket::open(ws_url).map_err(|op| PuppeteerError::CannotStartWssConnection {
                reason: op.to_string(),
            })?;

        if matches!(ws_connection.state(), gloo_net::websocket::State::Open) {
            return Err(PuppeteerError::CannotStartWssConnection {
                reason: "Connection is not Open".into(),
            });
        }

        const BUFFER_SIZE: usize = 1024;

        let (mut ws_sender, ws_receiver) = ws_connection.split();
        // from server to client
        let (event_sender, event_receiver) =
            futures::channel::mpsc::channel::<PuppetEvent>(BUFFER_SIZE);
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
            event_receiver,
            command_sender,
        };

        Ok(res)
    }

    // Renderer polling from our MPSC. MPSC is polling from websocket.
    // Renderer is polling at 60hz while MSPC is polling at around 100hz or even more.
    // This leads to renderer processing older messages.
    // So, we have to accumulate everything here just to make sure that we have all the messages
    pub fn poll_events(&mut self) -> Vec<PuppetEvent> {
        let mut events = vec![];

        // "It is not recommended to call this function from inside of a future"
        while let Ok(Some(event)) = self.event_receiver.try_next() {
            events.push(event);
        }

        events
    }
}
