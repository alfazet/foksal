use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::{
    RawFilter,
    net::response::{EventNotif, Response},
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
    Sink,
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
    pub filters: Option<Vec<RawFilter>>,
    pub group_by: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawUniqueArgs {
    pub tag: String,
    pub group_by: Option<Vec<String>>,
    pub sort: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawCoverArtArgs {
    pub uri: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAddToQueueArgs {
    pub uris: Vec<PathBuf>,
    pub pos: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRemoveFromQueueArgs {
    pub pos: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawQueueMoveArgs {
    pub from: usize,
    pub to: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAddAndPlayArgs {
    pub uris: Vec<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPlayArgs {
    pub pos: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVolumeChangeArgs {
    pub delta: i8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVolumeSetArgs {
    pub volume: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSeekByArgs {
    pub seconds: isize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSeekToArgs {
    pub seconds: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawDbRequest {
    Subscribe(DbSubTarget),
    Unsubscribe(DbSubTarget),
    Metadata(RawMetadataArgs),
    Select(RawSelectArgs),
    Unique(RawUniqueArgs),
    CoverArt(RawCoverArtArgs),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawPlayerRequest {
    Subscribe(PlayerSubTarget),
    Unsubscribe(PlayerSubTarget),
    AddToQueue(RawAddToQueueArgs),
    RemoveFromQueue(RawRemoveFromQueueArgs),
    QueueMove(RawQueueMoveArgs),
    AddAndPlay(RawAddAndPlayArgs),
    Play(RawPlayArgs),
    SeekBy(RawSeekByArgs),
    SeekTo(RawSeekToArgs),
    VolumeChange(RawVolumeChangeArgs),
    VolumeSet(RawVolumeSetArgs),
    State,
    Pause,
    Resume,
    Toggle,
    Stop,
    Next,
    Prev,
    QueueSeq,
    QueueLoop,
    QueueRandom,
    QueueSingle,
    QueueClear,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawFileRequest {
    GetChunk {
        uri: PathBuf,
        start: usize,
        end: usize,
    },
}

pub struct FileRequest {
    pub raw: RawFileRequest,
    pub respond_to: oneshot::Sender<Bytes>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum LocalRequestKind {
    DbRequest(RawDbRequest),
    PlayerRequest(RawPlayerRequest),
}

#[derive(Deserialize)]
pub struct LocalRequest {
    #[serde(flatten)]
    pub kind: LocalRequestKind,
    pub token: Option<String>,
}

pub struct MprisRequest {
    pub kind: LocalRequestKind,
    pub respond_to: oneshot::Sender<Response>,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum RemoteRequest {
    DbRequest {
        request: RawDbRequest,
        client: SocketAddr,
        token: Option<String>,
    },
    FileRequest(RawFileRequest),
}

impl SubTarget for DbSubTarget {}

impl SubTarget for PlayerSubTarget {}

impl<T: SubTarget> SubscribeArgs<T> {
    pub fn new(
        target: T,
        addr: SocketAddr,
        send_to: tokio_chan::UnboundedSender<EventNotif>,
    ) -> Self {
        Self {
            target,
            addr,
            send_to,
        }
    }
}

impl<T: SubTarget> UnsubscribeArgs<T> {
    pub fn new(target: T, addr: SocketAddr) -> Self {
        Self { target, addr }
    }
}

impl RawDbRequestArgs for RawMetadataArgs {}

impl RawDbRequestArgs for RawSelectArgs {}

impl RawDbRequestArgs for RawUniqueArgs {}

impl RawDbRequestArgs for RawCoverArtArgs {}

impl RawPlayerRequestArgs for RawAddToQueueArgs {}

impl RawPlayerRequestArgs for RawRemoveFromQueueArgs {}

impl RawPlayerRequestArgs for RawQueueMoveArgs {}

impl RawPlayerRequestArgs for RawPlayArgs {}

impl RawPlayerRequestArgs for RawAddAndPlayArgs {}

impl RawPlayerRequestArgs for RawVolumeChangeArgs {}

impl RawPlayerRequestArgs for RawVolumeSetArgs {}

impl RawPlayerRequestArgs for RawSeekByArgs {}

impl RawPlayerRequestArgs for RawSeekToArgs {}

impl FileRequest {
    pub fn new(raw: RawFileRequest, respond_to: oneshot::Sender<Bytes>) -> Self {
        Self { raw, respond_to }
    }
}

impl RemoteRequest {
    pub fn to_bytes(&self) -> Result<Bytes> {
        let s = serde_json::to_vec(&self)?;
        Ok(s.into())
    }
}
