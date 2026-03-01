use anyhow::Result;
use erased_serde::Serialize as ErasedSerialize;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::net::SocketAddr;
use tokio_tungstenite::tungstenite::{Bytes, Utf8Bytes};

use crate::net::core::JsonObject;

#[derive(Debug, Deserialize, Serialize)]
pub struct Response(JsonObject);

#[derive(Debug, Deserialize, Serialize)]
pub struct EventNotif {
    pub value: Value,
    pub subscriber: SocketAddr,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RemoteResponseInner {
    Response(Response),
    EventNotif(EventNotif),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RemoteResponse {
    pub inner: RemoteResponseInner,
    pub client: Option<SocketAddr>, // option, because the request might be unparseable
}

pub enum RemoteResponseKind {
    TextResponse(RemoteResponse),
    BinaryResponse(Vec<u8>),
}

impl Default for Response {
    fn default() -> Self {
        Self(JsonObject::new())
    }
}

impl<T> From<Result<T>> for Response {
    fn from(result: Result<T>) -> Self {
        match result {
            Ok(_) => Self::new_ok(),
            Err(e) => Self::new_err(e.to_string()),
        }
    }
}

impl Response {
    pub fn inner_mut(&mut self) -> &'_ mut JsonObject {
        &mut self.0
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        let s = serde_json::to_vec(&self.0)?;
        Ok(s.into())
    }

    pub fn new_ok() -> Self {
        Self(Map::from_iter([("ok".into(), Value::Bool(true))]))
    }

    pub fn new_err(reason: impl Into<String>) -> Self {
        Self(Map::from_iter([
            ("ok".into(), Value::Bool(false)),
            ("reason".into(), Value::String(reason.into())),
        ]))
    }

    pub fn version() -> Self {
        let version = format!("foksal v{}", env!("CARGO_PKG_VERSION"));
        let json = JsonObject::from_iter([("version".into(), Value::String(version))]);

        Self(json)
    }

    pub fn with_item(mut self, key: impl Into<String>, value: &dyn ErasedSerialize) -> Self {
        let value = match serde_json::to_value(value) {
            Ok(value) => value,
            Err(_) => return self,
        };
        self.inner_mut().insert(key.into(), value);

        self
    }
}

impl EventNotif {
    pub fn new(event: impl Serialize, subscriber: SocketAddr) -> Self {
        Self {
            value: serde_json::to_value(event).unwrap(),
            subscriber,
        }
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        let s = serde_json::to_vec(&self.value)?;
        Ok(s.into())
    }

    pub fn to_text(&self) -> Result<Utf8Bytes> {
        let s = serde_json::to_string(&self.value)?;
        Ok(s.into())
    }
}

impl RemoteResponse {
    pub fn new(inner: RemoteResponseInner, client: Option<SocketAddr>) -> Self {
        Self { inner, client }
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        let s = serde_json::to_vec(&self)?;
        Ok(s.into())
    }

    pub fn to_text(&self) -> Result<Utf8Bytes> {
        let s = serde_json::to_string(&self)?;
        Ok(s.into())
    }
}
