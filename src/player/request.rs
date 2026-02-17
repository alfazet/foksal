use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{net::SocketAddr, path::PathBuf};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::{
    net::{
        request::{
            PlayerSubTarget, RawAddToQueueArgs, RawFileRequest, RawPlayArgs, RawPlayerRequest,
            SubTarget, SubscribeArgs, UnsubscribeArgs,
        },
        response::{EventNotif, Response},
    },
    player::core::{Player, PlayerEvent},
};

pub trait ParsedPlayerRequestArgs {}

pub struct ParsedAddToQueueArgs {
    pub uri: PathBuf,
    pub pos: Option<usize>,
}

pub struct ParsedPlayArgs {
    pub uri: PathBuf,
}

pub enum PlayerRequestKind {
    Raw(RawPlayerRequest),
    Subscribe(SubscribeArgs<PlayerSubTarget>),
    Unsubscribe(UnsubscribeArgs<PlayerSubTarget>),
}

pub struct PlayerRequest {
    pub kind: PlayerRequestKind,
    pub respond_to: oneshot::Sender<Response>,
}

pub struct FileRequest {
    pub raw: RawFileRequest,
    pub respond_to: Option<oneshot::Sender<Bytes>>,
}

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

impl ParsedPlayerRequestArgs for ParsedAddToQueueArgs {}

impl ParsedPlayerRequestArgs for ParsedPlayArgs {}

impl TryFrom<RawAddToQueueArgs> for ParsedAddToQueueArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawAddToQueueArgs) -> Result<Self> {
        Ok(Self {
            uri: raw.uri,
            pos: raw.pos,
        })
    }
}

impl TryFrom<RawPlayArgs> for ParsedPlayArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawPlayArgs) -> Result<Self> {
        Ok(Self { uri: raw.uri })
    }
}

impl PlayerRequest {
    pub fn new(kind: PlayerRequestKind, respond_to: oneshot::Sender<Response>) -> Self {
        Self { kind, respond_to }
    }
}

impl FileRequest {
    pub fn new(raw: RawFileRequest, respond_to: Option<oneshot::Sender<Bytes>>) -> Self {
        Self { raw, respond_to }
    }
}

impl Player {
    /// adds the song pointed to by `uri` to the playback queue at position `pos` (0-indexed)
    /// to add to the end of the queue, don't specify `pos`
    pub fn add_to_queue(
        &mut self,
        ParsedAddToQueueArgs { uri, pos }: ParsedAddToQueueArgs,
    ) -> Response {
        self.add_to_queue_inner(uri, pos).into()
    }

    /// TODO: add more fields
    /// returns the player's state
    /// response format:
    /// ```json
    /// {
    ///     "ok": true,
    ///     "current_song": "current/song",
    ///     "queue_pos": 0,
    ///     "queue": [
    ///         "current/song",
    ///         "some/other/song"
    ///     ],
    /// }
    /// ```
    pub fn state(&self) -> Response {
        let queue = self.queue();
        Response::new_ok()
            .with_item("queue", &queue.list())
            .with_item("current_song", &queue.cur())
            .with_item("queue_pos", &queue.pos())
    }

    pub fn pause(&self) -> Response {
        self.pause_sink();
        Response::new_ok()
    }

    pub fn resume(&self) -> Response {
        self.resume_sink();
        Response::new_ok()
    }

    pub fn play(&self, ParsedPlayArgs { uri }: ParsedPlayArgs) -> Response {
        self.play_sink(uri);
        Response::new_ok()
    }
}
