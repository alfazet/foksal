use thiserror::Error;

#[derive(Debug, Error)]
pub enum FoksalError {
    #[error("websocket connection failed ({0})")]
    WsConnectionFailed(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("websocket handshake failed")]
    WsHandshakeFailed,
    #[error("tcp connection failed ({0})")]
    TcpConnectionFailed(#[from] std::io::Error),
    #[error("address parsing failed ({0})")]
    AddrParsingFailed(#[from] std::net::AddrParseError),
    #[error("connection timed out ({0})")]
    TimeoutError(#[from] tokio::time::error::Elapsed),
    #[error("serialization error ({0})")]
    SerializationError(#[from] serde_json::Error),
    #[error("server error ({0})")]
    ServerError(String),
    #[error("protocol error ({0})")]
    ProtocolError(String),
    #[error("disconnected from the foksal instance")]
    Disconnected,
    #[error("invalid tag value (should be null, string or number)")]
    InvalidTagValue,
}
