use anyhow::Result;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use crate::player::queue::Queue;

pub struct Player {
    music_root: PathBuf,
    queue: Queue,
}

impl Player {
    pub fn new(music_root: impl Into<PathBuf>) -> Self {
        Self {
            music_root: music_root.into(),
            queue: Queue::new(),
        }
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn add_to_queue_inner(
        &mut self,
        uri: impl Into<PathBuf>,
        pos: Option<usize>,
    ) -> Result<()> {
        match pos {
            Some(pos) => self.queue.insert(uri, pos),
            None => {
                self.queue.push(uri);
                Ok(())
            }
        }
    }
}
