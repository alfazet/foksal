use anyhow::Result;
use crossbeam_channel as cbeam_chan;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc as tokio_chan;

use crate::{
    net::{request::PlayerSubTarget, response::EventNotif},
    player::{queue::Queue, request::ParsedAddToQueueArgs, sink::SinkRequest},
};

type PlayerSubscribersMap =
    HashMap<(PlayerSubTarget, SocketAddr), tokio_chan::UnboundedSender<EventNotif>>;

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlayerEvent {
    Queue { queue: Vec<PathBuf> },
}

pub struct Player {
    queue: Queue,
    subscribers: PlayerSubscribersMap,
    tx_sink_request: cbeam_chan::Sender<SinkRequest>,
}

impl Player {
    pub fn new(tx_sink_request: cbeam_chan::Sender<SinkRequest>) -> Self {
        Self {
            queue: Queue::new(),
            subscribers: HashMap::new(),
            tx_sink_request,
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
            let (sub_target, sub_addr) = sub;
            if *sub_target == target {
                let _ = send_to.send(EventNotif::new(event.clone(), *sub_addr));
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

    pub fn play_sink(&self, uri: PathBuf) {
        let _ = self.tx_sink_request.send(SinkRequest::Play(uri));
        // TODO: notify subscribers to sink events
    }

    pub fn pause_sink(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Pause);
        // TODO: notify subscribers to sink evenets
    }

    pub fn resume_sink(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Resume);
        // TODO: notify subscribers to sink evenets
    }
}
