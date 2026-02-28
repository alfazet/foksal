use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::oneshot;

use crate::core::Player;
use libfoksalcommon::net::{
    request::{
        PlayerSubTarget, RawAddToQueueArgs, RawPlayArgs, RawPlayerRequest, RawRemoveFromQueueArgs,
        RawSeekArgs, RawVolumeArgs, SubscribeArgs, UnsubscribeArgs,
    },
    response::Response,
};

pub trait ParsedPlayerRequestArgs {}

pub struct ParsedAddToQueueArgs {
    pub uri: PathBuf,
    pub pos: Option<usize>,
}

pub struct ParsedRemoveFromQueueArgs {
    pub pos: usize,
}

pub struct ParsedPlayArgs {
    pub pos: usize,
}

pub struct ParsedVolumeArgs {
    pub delta: i8,
}

pub struct ParsedSeekArgs {
    pub seconds: isize,
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

impl ParsedPlayerRequestArgs for ParsedRemoveFromQueueArgs {}

impl ParsedPlayerRequestArgs for ParsedPlayArgs {}

impl ParsedPlayerRequestArgs for ParsedVolumeArgs {}

impl ParsedPlayerRequestArgs for ParsedSeekArgs {}

impl TryFrom<RawAddToQueueArgs> for ParsedAddToQueueArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawAddToQueueArgs) -> Result<Self> {
        Ok(Self {
            uri: raw.uri,
            pos: raw.pos,
        })
    }
}

impl TryFrom<RawRemoveFromQueueArgs> for ParsedRemoveFromQueueArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawRemoveFromQueueArgs) -> Result<Self> {
        Ok(Self { pos: raw.pos })
    }
}

impl TryFrom<RawPlayArgs> for ParsedPlayArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawPlayArgs) -> Result<Self> {
        Ok(Self { pos: raw.pos })
    }
}

impl TryFrom<RawVolumeArgs> for ParsedVolumeArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawVolumeArgs) -> Result<Self> {
        Ok(Self { delta: raw.delta })
    }
}

impl TryFrom<RawSeekArgs> for ParsedSeekArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawSeekArgs) -> Result<Self> {
        Ok(Self {
            seconds: raw.seconds,
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
    pub fn req_add_to_queue(
        &mut self,
        ParsedAddToQueueArgs { uri, pos }: ParsedAddToQueueArgs,
    ) -> Response {
        self.add_to_queue(uri, pos).into()
    }

    /// removes the song at position `pos` from the queue
    pub fn req_remove_from_queue(
        &mut self,
        ParsedRemoveFromQueueArgs { pos }: ParsedRemoveFromQueueArgs,
    ) -> Response {
        self.remove_from_queue(pos).into()
    }

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
    ///     "sink_state": "paused",
    ///     "volume": 80,
    ///     "elapsed": 123,
    /// }
    /// ```
    pub async fn req_state(&self) -> Response {
        let queue = self.queue();
        let cur_song = self.cur_song().await;
        let sink_state = self.sink_state().await;
        let volume = self.volume().await;
        let elapsed = self.elapsed().await;
        Response::new_ok()
            .with_item("queue", &queue.list())
            .with_item("current_song", &cur_song)
            .with_item("queue_pos", &queue.pos())
            .with_item("sink_state", &sink_state)
            .with_item("volume", &volume)
            .with_item("elapsed", &elapsed)
    }

    pub fn req_play(&mut self, ParsedPlayArgs { pos }: ParsedPlayArgs) -> Response {
        self.play(pos).into()
    }

    pub fn req_volume(&self, ParsedVolumeArgs { delta }: ParsedVolumeArgs) -> Response {
        self.change_volume(delta);
        Response::new_ok()
    }

    pub fn req_seek(&self, ParsedSeekArgs { seconds }: ParsedSeekArgs) -> Response {
        self.seek(seconds);
        Response::new_ok()
    }

    pub fn req_pause(&self) -> Response {
        self.pause();
        Response::new_ok()
    }

    pub fn req_resume(&self) -> Response {
        self.resume();
        Response::new_ok()
    }

    pub fn req_toggle(&self) -> Response {
        self.toggle();
        Response::new_ok()
    }

    pub fn req_stop(&self) -> Response {
        self.stop();
        Response::new_ok()
    }

    pub fn req_next(&mut self) -> Response {
        self.next();
        Response::new_ok()
    }

    pub fn req_prev(&mut self) -> Response {
        self.prev();
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
