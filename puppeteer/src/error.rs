use thiserror::Error;

#[derive(Debug, Error)]
pub enum PuppeteerError {
    #[error("Cannot start wss connection: {reason}")]
    CannotStartWssConnection { reason: String },
    #[error("Cannot encode message: {reason}")]
    CannotEncodeMessage { reason: String },
    #[error("Cannot decode message: {source}")]
    CannotDecodeMessage {
        #[source]
        source: rmp_serde::decode::Error,
    },
    #[error("Cannot send command")]
    CannotSendCommand,
}
