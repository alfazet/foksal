use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FoksalError {
    #[error("websocket error ({0})")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("address parsing failed ({0})")]
    AddrParse(#[from] std::net::AddrParseError),
    #[error("connection timed out ({0})")]
    Timeout(#[from] tokio::time::error::Elapsed),
    #[error("(de)serialization error ({0})")]
    Serialization(#[from] serde_json::Error),
    #[error("base64 decoding failed ({0})")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("server rejected request: {reason}")]
    ServerError { reason: String },
    #[error("unexpected response shape for `{request}`")]
    UnexpectedResponse { request: &'static str },
    #[error("major version mismatch: libfoksalclient {lib_version} vs foksal {instance_version}")]
    VersionMismatch {
        lib_version: String,
        instance_version: String,
    },
    #[error("invalid welcome message received")]
    InvalidWelcome,
    #[error("invalid tag value: expected null, string, or number, got {0}")]
    InvalidTagValue(String),
    #[error("disconnected from the foksal instance")]
    Disconnected,
    #[error("{error}, reason: {reason}")]
    Async { error: String, reason: String },
}
