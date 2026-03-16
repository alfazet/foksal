use thiserror::Error;

#[derive(Debug, Error)]
pub enum FoksalError {
    #[error("websocket connection failed ({0})")]
    WsConnectionError(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("address parsing failed ({0})")]
    AddrParsingError(#[from] std::net::AddrParseError),
    #[error("connection timed out ({0})")]
    TimeoutError(#[from] tokio::time::error::Elapsed),
    #[error("(de)serialization error ({0})")]
    SerializationError(#[from] serde_json::Error),
    #[error("foksal protocol error ({0})")]
    ProtocolError(String),
    #[error("invalid tag value (should be null, string or number)")]
    InvalidTagValue,
    #[error("disconnected from the foksal instance")]
    Disconnected,
}
