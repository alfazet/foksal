use anyhow::Result;
use crossbeam_channel as cbeam_chan;
use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf};
use tokio::sync::mpsc as tokio_chan;

use crate::{queue::Queue, sink::SinkRequest};
use libfoksalcommon::net::{request::PlayerSubTarget, response::EventNotif};

type PlayerSubscribersMap =
    HashMap<(PlayerSubTarget, SocketAddr), tokio_chan::UnboundedSender<EventNotif>>;

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlayerEvent {
    QueueContent { queue: Vec<PathBuf> },
    QueuePos { pos: Option<usize> },
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

    pub fn add_to_queue(&mut self, uri: impl Into<PathBuf>, pos: Option<usize>) -> Result<()> {
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
                PlayerEvent::QueueContent {
                    queue: self.queue.list_cloned(),
                },
            );
        }

        res
    }

    pub fn play(&mut self, pos: usize) -> Result<()> {
        self.queue.move_to(pos)?;
        let uri = self.queue.get(pos)?;
        self.play_from_uri(uri);
        self.notify_subscribers(
            PlayerSubTarget::Queue,
            PlayerEvent::QueuePos {
                pos: self.queue.pos(),
            },
        );

        Ok(())
    }

    pub fn pause(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Pause);
        // TODO: notify subscribers to sink evenets
    }

    pub fn resume(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Resume);
        // TODO: notify subscribers to sink evenets
    }

    pub fn toggle(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Toggle);
        // TODO: notify subscribers to sink evenets
    }

    pub fn stop(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Stop);
        // TODO: notify subscribers to sink evenets
    }

    pub fn next(&mut self) {
        self.queue.move_to_next();
        match self.queue.cur() {
            Some(uri) => self.play_from_uri(uri),
            None => self.stop(),
        }

        self.notify_subscribers(
            PlayerSubTarget::Queue,
            PlayerEvent::QueuePos {
                pos: self.queue.pos(),
            },
        );
    }

    pub fn prev(&mut self) {
        self.queue.move_to_prev();
        match self.queue.cur() {
            Some(uri) => self.play_from_uri(uri),
            None => self.stop(),
        }

        self.notify_subscribers(
            PlayerSubTarget::Queue,
            PlayerEvent::QueuePos {
                pos: self.queue.pos(),
            },
        );
    }

    fn play_from_uri(&self, uri: impl Into<PathBuf>) {
        let _ = self.tx_sink_request.send(SinkRequest::Play(uri.into()));
        // TODO: notify subscribers to sink evenets
    }

    fn notify_subscribers(&self, target: PlayerSubTarget, event: PlayerEvent) {
        for (sub, send_to) in self.subscribers.iter() {
            let (sub_target, sub_addr) = sub;
            if *sub_target == target {
                let _ = send_to.send(EventNotif::new(event.clone(), *sub_addr));
            }
        }
    }
}
