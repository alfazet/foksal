use anyhow::{Result, bail};
use serde_json::Value;
use std::{net::SocketAddr, path::PathBuf};
use tokio::sync::{mpsc as tokio_chan, oneshot};

use crate::{
    net::{
        request::{RawAddToQueueArgs, RawPlayerRequest, SubTarget},
        response::{EventNotif, Response},
    },
    player::core::{Player, PlayerEvent},
};

pub struct SubscribeArgs {
    pub target: SubTarget,
    pub addr: SocketAddr,
    pub send_to: tokio_chan::UnboundedSender<EventNotif>,
}

pub struct UnsubscribeArgs {
    pub target: SubTarget,
    pub addr: SocketAddr,
}

pub trait ParsedPlayerRequestArgs {}

pub struct ParsedAddToQueueArgs {
    pub uri: PathBuf,
    pub pos: Option<usize>,
}

impl ParsedPlayerRequestArgs for ParsedAddToQueueArgs {}

pub enum PlayerRequestKind {
    Raw(RawPlayerRequest),
    Subscribe(SubscribeArgs),
    Unsubscribe(UnsubscribeArgs),
}

pub struct PlayerRequest {
    pub kind: PlayerRequestKind,
    pub respond_to: oneshot::Sender<Response>,
}

impl SubscribeArgs {
    pub fn new(
        target: SubTarget,
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

impl UnsubscribeArgs {
    pub fn new(target: SubTarget, addr: SocketAddr) -> Self {
        Self { target, addr }
    }
}

impl TryFrom<RawAddToQueueArgs> for ParsedAddToQueueArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawAddToQueueArgs) -> Result<Self> {
        Ok(Self {
            uri: raw.uri,
            pos: raw.pos,
        })
    }
}

impl PlayerRequest {
    pub fn new(kind: PlayerRequestKind, respond_to: oneshot::Sender<Response>) -> Self {
        Self { kind, respond_to }
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
}
