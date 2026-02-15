use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::{
    db::filter::RawFilter,
    net::{
        core::*,
        response::{EventNotif, Response},
    },
};

pub trait RawDbRequestArgs {}

pub trait RawPlayerRequestArgs {}

pub trait SubTarget {}

pub struct SubscribeArgs<T: SubTarget> {
    pub target: T,
    pub addr: SocketAddr,
    pub send_to: tokio_chan::UnboundedSender<EventNotif>,
}

pub struct UnsubscribeArgs<T: SubTarget> {
    pub target: T,
    pub addr: SocketAddr,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum DbSubTarget {
    Update,
}

#[derive(Copy, Clone, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum PlayerSubTarget {
    Queue,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawMetadataArgs {
    pub uris: Vec<PathBuf>,
    pub tags: Vec<String>,
}

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawDbRequest {
    Subscribe(DbSubTarget),
    Unsubscribe(DbSubTarget),
    Metadata(RawMetadataArgs),
    Select(RawSelectArgs),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawPlayerRequest {
    Subscribe(PlayerSubTarget),
    Unsubscribe(PlayerSubTarget),
    AddToQueue(RawAddToQueueArgs),
    State,
    Play,
    Toggle,
    Next,
    Prev,
}

// #[derive(Deserialize)]
// #[serde(tag = "kind", rename_all = "snake_case")]
// pub enum RawFileRequest {
//     FetchChunk(RawFetchChunkArgs),
// }

#[derive(Deserialize)]
#[serde(untagged)]
pub enum LocalRequestKind {
    DbRequest(RawDbRequest),
    PlayerRequest(RawPlayerRequest),
}

#[derive(Deserialize, Serialize)]
pub enum RemoteRequestKind {
    DbRequest {
        request: RawDbRequest,
        client: SocketAddr,
    },
    // FileRequest(RawFileRequest),
}

impl SubTarget for DbSubTarget {}

impl SubTarget for PlayerSubTarget {}

impl RawDbRequestArgs for RawMetadataArgs {}

impl RawDbRequestArgs for RawSelectArgs {}

impl RawPlayerRequestArgs for RawAddToQueueArgs {}
