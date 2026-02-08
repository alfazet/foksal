use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Bytes;

use crate::net::response::Response;

#[derive(Deserialize)]
pub struct MetadataArgs {
    pub uris: Vec<PathBuf>,
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestKind {
    Metadata(MetadataArgs),
}

pub struct ParsedRequest {
    pub kind: RequestKind,
    pub respond_to: oneshot::Sender<Response>,
}

pub struct RawRequest {
    pub data: Bytes,
    pub respond_to: oneshot::Sender<Response>,
}

impl RawRequest {
    pub fn new(data: impl Into<Bytes>, respond_to: oneshot::Sender<Response>) -> Self {
        Self {
            data: data.into(),
            respond_to,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
