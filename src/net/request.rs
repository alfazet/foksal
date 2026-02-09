use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Bytes;

use crate::net::{core::JsonObject, response::Response};

pub trait Request {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataArgs {
    pub uris: Vec<PathBuf>,
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DbRequest {
    Metadata(MetadataArgs),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RequestKind {
    DbRequest(DbRequest),
}

pub struct RawRequest {
    pub data: Bytes,
    pub respond_to: oneshot::Sender<Bytes>,
}

pub struct ParsedRequest<T: Request> {
    pub request: T,
    pub respond_to: oneshot::Sender<Response>,
}

impl Request for DbRequest {}

impl RawRequest {
    pub fn new(data: impl Into<Bytes>, respond_to: oneshot::Sender<Bytes>) -> Self {
        Self {
            data: data.into(),
            respond_to,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl<T> ParsedRequest<T>
where
    T: Request,
{
    pub fn new(request: T, respond_to: oneshot::Sender<Response>) -> Self {
        Self {
            request,
            respond_to,
        }
    }
}
