use anyhow::Result;
use crossbeam_channel as cbeam_chan;
use serde::Serialize;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::sync::{mpsc as tokio_chan, oneshot};

use crate::{
    queue::Queue,
    sink::{SinkRequest, SinkState},
};
use libfoksalcommon::net::{request::PlayerSubTarget, response::EventNotif};

type PlayerSubscribersMap =
    HashMap<(PlayerSubTarget, SocketAddr), tokio_chan::UnboundedSender<EventNotif>>;

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlayerEvent {
    QueueContent { queue: Vec<PathBuf> },
    QueuePos { pos: Option<usize> },
    CurrentSong { uri: PathBuf },
    SinkState { state: SinkState },
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

    pub async fn pause(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Pause);
        let state = self.sink_state().await;
        self.notify_subscribers(PlayerSubTarget::Sink, PlayerEvent::SinkState { state });
    }

    pub async fn resume(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Resume);
        let state = self.sink_state().await;
        self.notify_subscribers(PlayerSubTarget::Sink, PlayerEvent::SinkState { state });
    }

    pub async fn toggle(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Toggle);
        let state = self.sink_state().await;
        self.notify_subscribers(PlayerSubTarget::Sink, PlayerEvent::SinkState { state });
    }

    pub async fn stop(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Stop);
        let state = self.sink_state().await;
        self.notify_subscribers(PlayerSubTarget::Sink, PlayerEvent::SinkState { state });
    }

    pub async fn next(&mut self) {
        self.queue.move_to_next();
        match self.queue.cur() {
            Some(uri) => self.play_from_uri(uri),
            None => self.stop().await,
        }

        self.notify_subscribers(
            PlayerSubTarget::Queue,
            PlayerEvent::QueuePos {
                pos: self.queue.pos(),
            },
        );
    }

    pub async fn prev(&mut self) {
        self.queue.move_to_prev();
        match self.queue.cur() {
            Some(uri) => self.play_from_uri(uri),
            None => self.stop().await,
        }

        self.notify_subscribers(
            PlayerSubTarget::Queue,
            PlayerEvent::QueuePos {
                pos: self.queue.pos(),
            },
        );
    }

    async fn sink_state(&self) -> SinkState {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx_sink_request.send(SinkRequest::GetState(tx));
        rx.await.unwrap_or_default()
    }

    fn play_from_uri(&self, uri: impl AsRef<Path>) {
        let _ = self
            .tx_sink_request
            .send(SinkRequest::Play(uri.as_ref().to_path_buf()));
        self.notify_subscribers(
            PlayerSubTarget::Sink,
            PlayerEvent::CurrentSong {
                uri: uri.as_ref().to_path_buf(),
            },
        );
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
