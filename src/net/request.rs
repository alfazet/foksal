use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::oneshot;

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
    kind: RequestKind,
    respond_to: oneshot::Sender<Response>,
}

pub struct RawRequest {
    data: Vec<u8>,
    respond_to: oneshot::Sender<Response>,
}
