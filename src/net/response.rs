use anyhow::Result;
use erased_serde::Serialize as ErasedSerialize;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    fmt::{self, Display, Formatter},
    net::SocketAddr,
    path::PathBuf,
};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Bytes;

use crate::{net::core::JsonObject, player::core::PlayerEvent};

#[derive(Debug, Deserialize, Serialize)]
pub struct Response(JsonObject);

#[derive(Debug, Deserialize, Serialize)]
pub struct EventNotif {
    value: Value,
    subscriber: SocketAddr,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RemoteResponseInner {
    Response(Response),
    EventNotif(EventNotif),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RemoteResponse {
    inner: RemoteResponseInner,
    client: Option<SocketAddr>, // option, because the request might be unparseable
}

pub enum RemoteResponseKind {
    Response(RemoteResponse),
    // Chunk(Vec< whatever the type of samples will be>),
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
        let s = serde_json::to_string(&self.0)?;
        Ok(s.as_bytes().to_vec().into())
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
        let version_info = format!("foksal v{}", env!("CARGO_PKG_VERSION"));
        let json = JsonObject::from_iter([
            ("ok".into(), Value::Bool(true)),
            ("version".into(), Value::String(version_info)),
        ]);

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

    pub fn to_bytes_local(&self) -> Result<Bytes> {
        let s = serde_json::to_string(&self.value)?; // TODO: use to_vec()?
        Ok(s.as_bytes().to_vec().into())
    }

    pub fn to_bytes_remote(&self) -> Result<Bytes> {
        let s = serde_json::to_string(&self)?;
        Ok(s.as_bytes().to_vec().into())
    }
}

impl RemoteResponse {
    pub fn new(inner: RemoteResponseInner, client: Option<SocketAddr>) -> Self {
        Self { inner, client }
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        let s = serde_json::to_string(&self)?;
        Ok(s.as_bytes().to_vec().into())
    }
}

impl RemoteResponseKind {
    pub fn to_bytes(&self) -> Result<Bytes> {
        match self {
            Self::Response(response) => response.to_bytes(),
            // Self::Chunk => add some bytes to the beginning as a marker
        }
    }
}
