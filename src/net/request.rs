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
pub enum DbRequest {
    Metadata(RawMetadataArgs),
    Select(RawSelectArgs),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerRequest {
    AddToQueue(RawAddToQueueArgs),
    State,
    Play,
    Toggle,
    Next,
    Prev,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum LocalRequestKind {
    DbRequest(DbRequest),
    PlayerRequest(PlayerRequest),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ProxyRequestKind {
    DbRequest(DbRequest),
    PlayerRequest(PlayerRequest),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum HeadlessRequestKind {
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

impl Request for PlayerRequest {}

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

impl RawDbRequestArgs for RawMetadataArgs {}

impl RawDbRequestArgs for RawSelectArgs {}

impl RawPlayerRequestArgs for RawAddToQueueArgs {}
