use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Bytes;

use crate::{
    db::filter::RawFilter,
    net::{core::*, response::Response},
};

pub trait Request {}

pub trait RawDbRequestArgs {}

pub trait RawPlayerRequestArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMetadataArgs {
    pub uris: Vec<PathBuf>,
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSelectArgs {
    pub filters: Vec<RawFilter>,
    pub group_by: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAddToQueueArgs {
    pub uri: PathBuf,
    pub pos: Option<usize>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawDbRequest {
    Metadata(RawMetadataArgs),
    Select(RawSelectArgs),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawPlayerRequest {
    Subscribe(SubTarget),
    Unsubscribe(SubTarget),
    AddToQueue(RawAddToQueueArgs),
    State,
    Play,
    Toggle,
    Next,
    Prev,
}

#[derive(Copy, Clone, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum SubTarget {
    Queue,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawFileRequest {
    FetchChunk(PathBuf), // TODO: change to RawFetchChunkArgs, timestamps too
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum LocalRequestKind {
    DbRequest(RawDbRequest),
    PlayerRequest(RawPlayerRequest),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RemoteRequestKind {
    DbRequest(RawDbRequest),
    FileRequest(RawFileRequest),
}

pub struct ParsedRequest<T: Request> {
    pub request: T,
    pub respond_to: oneshot::Sender<Response>,
}

impl Request for RawDbRequest {}

impl Request for RawPlayerRequest {}

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

impl RawDbRequestArgs for RawMetadataArgs {}

impl RawDbRequestArgs for RawSelectArgs {}

impl RawPlayerRequestArgs for RawAddToQueueArgs {}
