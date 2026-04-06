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
    Volume,
    queue::{Queue, QueueMode},
    sink::{PlaybackState, SinkRequest},
};
use libfoksalcommon::net::{request::PlayerSubTarget, response::EventNotif};

type PlayerSubscribersMap =
    HashMap<(PlayerSubTarget, SocketAddr), tokio_chan::UnboundedSender<EventNotif>>;

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PlayerEvent {
    QueueContent { queue: Vec<PathBuf> },
    QueuePos { pos: Option<usize> },
    QueueMode { mode: QueueMode },
    CurrentSong { uri: PathBuf },
    PlaybackState { state: PlaybackState },
    Volume { volume: u8 },
    Elapsed { seconds: u64 },
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

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub async fn playback_state(&self) -> PlaybackState {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx_sink_request.send(SinkRequest::GetState(tx));
        rx.await.unwrap_or_default()
    }

    pub fn cur_song(&self) -> Option<&Path> {
        self.queue.cur().map(|c| c.0)
    }

    pub fn cur_id(&self) -> Option<usize> {
        self.queue.cur().map(|c| c.1)
    }

    pub async fn volume(&self) -> u8 {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx_sink_request.send(SinkRequest::GetVolume(tx));
        rx.await.unwrap_or_default().0
    }

    pub async fn elapsed(&self) -> u64 {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx_sink_request.send(SinkRequest::GetElapsed(tx));
        rx.await.unwrap_or_default()
    }

    pub fn add_to_queue(
        &mut self,
        uris: Vec<impl AsRef<Path> + Into<PathBuf>>,
        pos: Option<usize>,
    ) -> Result<()> {
        let res = match pos {
            Some(pos) => self.queue.insert(&uris, pos),
            None => {
                self.queue.push(&uris);
                Ok(())
            }
        };
        if res.is_ok() {
            self.notify_queue_content();
        }

        res
    }

    pub fn remove_from_queue(&mut self, pos: usize) -> Result<()> {
        if self.queue.pos().is_some_and(|p| p == pos) {
            self.stop();
        }
        let prev_pos = self.queue.pos();
        self.queue.remove(pos)?;
        self.notify_queue_content();
        if prev_pos != self.queue.pos() {
            self.notify_queue_pos();
        }

        Ok(())
    }

    pub fn queue_move(&mut self, from: usize, to: usize) -> Result<()> {
        self.queue.move_pos(from, to)?;
        self.notify_queue_content();
        self.notify_queue_pos();

        Ok(())
    }

    pub fn play(&mut self, pos: usize) -> Result<()> {
        self.queue.move_to(pos)?;
        let uri = self.queue.get(pos)?;
        self.play_from_uri(uri);
        self.notify_queue_pos();

        Ok(())
    }

    pub fn add_and_play(&mut self, uris: Vec<impl AsRef<Path> + Into<PathBuf>>) {
        self.queue.push_and_move_to(&uris);
        self.notify_queue_content();
        self.next();
    }

    pub fn volume_change(&self, delta: i8) {
        let _ = self.tx_sink_request.send(SinkRequest::VolChange(delta));
    }

    pub fn volume_set(&self, volume: u8) {
        let _ = self.tx_sink_request.send(SinkRequest::VolSet(volume));
    }

    pub fn seek_by(&self, seconds: isize) {
        let _ = self.tx_sink_request.send(SinkRequest::SeekBy(seconds));
    }

    pub fn seek_to(&self, seconds: usize) {
        let _ = self.tx_sink_request.send(SinkRequest::SeekTo(seconds));
    }

    pub fn pause(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Resume);
    }

    pub fn toggle(&self) {
        let _ = self.tx_sink_request.send(SinkRequest::Toggle);
    }

    pub fn stop(&mut self) {
        let _ = self.tx_sink_request.send(SinkRequest::Stop);
        self.queue.stop();
        self.notify_queue_pos();
    }

    pub fn next(&mut self) {
        self.queue.move_to_next();
        match self.queue.cur().map(|c| c.0) {
            Some(uri) => self.play_from_uri(uri),
            None => self.stop(),
        }
        self.notify_queue_pos();
    }

    pub fn prev(&mut self) {
        self.queue.move_to_prev();
        match self.queue.cur().map(|c| c.0) {
            Some(uri) => self.play_from_uri(uri),
            None => self.stop(),
        }
        self.notify_queue_pos();
    }

    pub fn queue_seq(&mut self) {
        self.queue.set_mode_seq();
        self.notify_queue_mode();
    }

    pub fn queue_loop(&mut self) {
        self.queue.set_mode_loop();
        self.notify_queue_mode();
    }

    pub fn queue_random(&mut self) {
        self.queue.set_mode_random();
        self.notify_queue_mode();
    }

    pub fn queue_clear(&mut self) {
        self.queue.clear();
        let _ = self.tx_sink_request.send(SinkRequest::Stop);
        self.notify_queue_pos();
        self.notify_queue_content();
    }

    pub fn notify_queue_pos(&self) {
        let pos = self.queue.pos();
        self.notify_subscribers(PlayerSubTarget::Queue, PlayerEvent::QueuePos { pos });
    }

    pub fn notify_queue_content(&self) {
        let queue = self.queue.list_cloned();
        self.notify_subscribers(PlayerSubTarget::Queue, PlayerEvent::QueueContent { queue });
    }

    pub fn notify_playback_state(&self, state: PlaybackState) {
        self.notify_subscribers(PlayerSubTarget::Sink, PlayerEvent::PlaybackState { state });
    }

    pub fn notify_queue_mode(&self) {
        let mode = self.queue.mode();
        self.notify_subscribers(PlayerSubTarget::Queue, PlayerEvent::QueueMode { mode });
    }

    pub fn notify_volume(&self, volume: Volume) {
        self.notify_subscribers(
            PlayerSubTarget::Sink,
            PlayerEvent::Volume { volume: volume.0 },
        );
    }

    pub fn notify_elapsed(&self, seconds: u64) {
        self.notify_subscribers(PlayerSubTarget::Sink, PlayerEvent::Elapsed { seconds });
    }

    pub fn notify_song(&self, uri: impl AsRef<Path>) {
        self.notify_subscribers(
            PlayerSubTarget::Sink,
            PlayerEvent::CurrentSong {
                uri: uri.as_ref().to_path_buf(),
            },
        );
    }

    fn play_from_uri(&self, uri: impl AsRef<Path>) {
        let _ = self
            .tx_sink_request
            .send(SinkRequest::Play(uri.as_ref().to_path_buf()));
        self.notify_song(uri);
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
