use anyhow::Result;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc as tokio_chan;

use crate::{
    net::{request::PlayerSubTarget, response::EventNotif},
    player::{queue::Queue, request::ParsedAddToQueueArgs},
};

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlayerEvent {
    Queue { queue: Vec<PathBuf> },
}

pub struct Player {
    music_root: PathBuf,
    queue: Queue,
    subscribers: HashMap<(PlayerSubTarget, SocketAddr), tokio_chan::UnboundedSender<EventNotif>>,
}

impl Player {
    pub fn new(music_root: impl Into<PathBuf>) -> Self {
        Self {
            music_root: music_root.into(),
            queue: Queue::new(),
            subscribers: HashMap::new(),
        }
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn add_subscriber(
        &mut self,
        target: PlayerSubTarget,
        addr: SocketAddr,
        send_to: tokio_chan::UnboundedSender<EventNotif>,
    ) {
        self.subscribers.insert((target, addr), send_to);
    }

    pub fn remove_subscriber(&mut self, target: PlayerSubTarget, addr: SocketAddr) {
        self.subscribers.remove(&(target, addr));
    }

    fn notify_subscribers(&self, target: PlayerSubTarget, event: PlayerEvent) {
        for (sub, send_to) in self.subscribers.iter() {
            let (sub_target, _) = sub;
            if *sub_target == target {
                let _ = send_to.send(EventNotif::from_player_event(event.clone()));
            }
        }
    }

    pub fn add_to_queue_inner(
        &mut self,
        uri: impl Into<PathBuf>,
        pos: Option<usize>,
    ) -> Result<()> {
        let res = match pos {
            Some(pos) => self.queue.insert(uri, pos),
            None => {
                self.queue.push(uri);
                Ok(())
            }
        };
        if res.is_ok() {
            self.notify_subscribers(
                PlayerSubTarget::Queue,
                PlayerEvent::Queue {
                    queue: self.queue.list_cloned(),
                },
            );
        }

        res
    }
}
