use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::oneshot;

use crate::core::Player;
use libfoksalcommon::net::{
    request::{
        PlayerSubTarget, RawAddToQueueArgs, RawPlayArgs, RawPlayerRequest, SubscribeArgs,
        UnsubscribeArgs,
    },
    response::Response,
};

pub trait ParsedPlayerRequestArgs {}

pub struct ParsedAddToQueueArgs {
    pub uri: PathBuf,
    pub pos: Option<usize>,
}

pub struct ParsedPlayArgs {
    pub pos: usize,
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
        Ok(Self { pos: raw.pos })
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
    pub fn req_add_to_queue(
        &mut self,
        ParsedAddToQueueArgs { uri, pos }: ParsedAddToQueueArgs,
    ) -> Response {
        self.add_to_queue(uri, pos).into()
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
    pub fn req_state(&self) -> Response {
        let queue = self.queue();
        Response::new_ok()
            .with_item("queue", &queue.list())
            .with_item("current_song", &queue.cur())
            .with_item("queue_pos", &queue.pos())
    }

    pub fn req_play(&mut self, ParsedPlayArgs { pos }: ParsedPlayArgs) -> Response {
        self.play(pos).into()
    }

    pub async fn req_pause(&self) -> Response {
        self.pause().await;
        Response::new_ok()
    }

    pub async fn req_resume(&self) -> Response {
        self.resume().await;
        Response::new_ok()
    }

    pub async fn req_toggle(&self) -> Response {
        self.toggle().await;
        Response::new_ok()
    }

    pub async fn req_stop(&self) -> Response {
        self.stop().await;
        Response::new_ok()
    }

    pub async fn req_next(&mut self) -> Response {
        self.next().await;
        Response::new_ok()
    }

    pub async fn req_prev(&mut self) -> Response {
        self.prev().await;
        Response::new_ok()
    }

    pub fn req_queue_seq(&mut self) -> Response {
        self.queue_seq();
        Response::new_ok()
    }

    pub fn req_queue_random(&mut self) -> Response {
        self.queue_random();
        Response::new_ok()
    }
}
