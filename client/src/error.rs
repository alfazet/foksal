use thiserror::Error;

#[derive(Debug, Error)]
pub enum FoksalError {
    #[error("connection failed: {0}")]
    ConnectionFailed(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("server error: {0}")]
    ServerError(String),
    #[error("protocol error: {0}")]
    ProtocolError(String),
    #[error("disconnected from the foksal instance")]
    Disconnected,
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
