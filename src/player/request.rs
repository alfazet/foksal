use anyhow::{Result, bail};
use serde_json::Value;
use std::path::PathBuf;

use crate::{
    net::{request::RawAddToQueueArgs, response::Response},
    player::core::Player,
};

pub trait ParsedPlayerRequestArgs {}

pub struct ParsedAddToQueueArgs {
    pub uri: PathBuf,
    pub pos: Option<usize>,
}

impl ParsedPlayerRequestArgs for ParsedAddToQueueArgs {}

impl TryFrom<RawAddToQueueArgs> for ParsedAddToQueueArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawAddToQueueArgs) -> Result<Self> {
        Ok(Self {
            uri: raw.uri,
            pos: raw.pos,
        })
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
