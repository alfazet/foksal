use anyhow::Result;
use globset::GlobSet;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use tokio::sync::mpsc as tokio_chan;

use crate::{fs_utils, fs_watcher, song_metadata::SongMetadata};
use libfoksalcommon::net::{request::DbSubTarget, response::EventNotif};

type Table = BTreeMap<PathBuf, SongMetadata>;

type DbSubscribersMap = HashMap<(DbSubTarget, SocketAddr), tokio_chan::UnboundedSender<EventNotif>>;

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DbEvent {
    Create { uri: PathBuf },
    Modify { uri: PathBuf },
    Remove { uri: PathBuf },
}

/// TODO:
/// - keep m3u playlists
/// - allow relative paths in requests
#[derive(Default)]
pub struct Db {
    pub table: Table,
    music_root: PathBuf,
    subscribers: DbSubscribersMap,
}

pub struct SharedDb {
    pub inner: Arc<RwLock<Db>>,
    music_root: PathBuf,
}

impl Db {
    pub fn new(
        music_root: impl AsRef<Path> + Into<PathBuf>,
        ignore_globset: &GlobSet,
        allowed_exts: &[impl AsRef<str>],
    ) -> Result<Self> {
        let uris = fs_utils::walk_dir(&music_root, ignore_globset.clone(), allowed_exts)?;
        let table = Self::init_table(uris);
        let music_root = music_root.as_ref().to_path_buf();
        let subscribers = HashMap::new();

        Ok(Self {
            table,
            music_root,
            subscribers,
        })
    }

    pub fn add_subscriber(
        &mut self,
        target: DbSubTarget,
        addr: SocketAddr,
        send_to: tokio_chan::UnboundedSender<EventNotif>,
    ) {
        self.subscribers.insert((target, addr), send_to);
    }

    pub fn remove_subscriber(&mut self, target: DbSubTarget, addr: SocketAddr) {
        self.subscribers.remove(&(target, addr));
    }

    fn notify_subscribers(&self, target: DbSubTarget, event: DbEvent) {
        for (sub, send_to) in self.subscribers.iter() {
            let (sub_target, sub_addr) = sub;
            if *sub_target == target {
                let _ = send_to.send(EventNotif::new(event.clone(), *sub_addr));
            }
        }
    }

    fn create(&mut self, uri: impl AsRef<Path> + Into<PathBuf>, data: SongMetadata) {
        self.table.insert(uri.into(), data);
    }

    fn modify(&mut self, uri: impl AsRef<Path>, new_data: SongMetadata) {
        if let Some(data) = self.table.get_mut(uri.as_ref()) {
            *data = new_data;
        }
    }

    fn remove(&mut self, uri: impl AsRef<Path>) -> Option<()> {
        self.table.remove(uri.as_ref()).map(|_| ())
    }

    fn init_table(uris: impl IntoParallelIterator<Item = PathBuf>) -> Table {
        uris.into_par_iter()
            .filter_map(move |uri| SongMetadata::try_new(&uri).ok().map(|data| (uri, data)))
            .collect()
    }
}

impl Clone for SharedDb {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            music_root: self.music_root.clone(),
        }
    }
}

impl SharedDb {
    pub fn new(db: Db) -> Self {
        let music_root = db.music_root.clone();

        Self {
            inner: Arc::new(RwLock::new(db)),
            music_root,
        }
    }

    /// starts the watcher daemon on a separate thread
    pub fn start_fs_watcher(
        &self,
        music_root: impl Into<PathBuf>,
        ignore_globset: GlobSet,
        allowed_exts: Vec<String>,
    ) -> Result<()> {
        fs_watcher::run(self.clone(), music_root, ignore_globset, allowed_exts)
    }

    pub fn create(&mut self, uri: impl AsRef<Path> + Into<PathBuf>) -> Result<()> {
        let data = SongMetadata::try_new(&uri)?;
        let mut db = self.inner.write().unwrap();
        db.create(uri.as_ref(), data);
        db.notify_subscribers(DbSubTarget::Update, DbEvent::Create { uri: uri.into() });

        Ok(())
    }

    pub fn modify(&mut self, uri: impl AsRef<Path> + Into<PathBuf>) -> Result<()> {
        let data = SongMetadata::try_new(&uri)?;
        let mut db = self.inner.write().unwrap();
        db.modify(uri.as_ref(), data);
        db.notify_subscribers(DbSubTarget::Update, DbEvent::Modify { uri: uri.into() });

        Ok(())
    }

    pub fn remove(&mut self, uri: impl AsRef<Path> + Into<PathBuf>) -> Option<()> {
        let mut db = self.inner.write().unwrap();
        db.remove(uri.as_ref())?;
        db.notify_subscribers(DbSubTarget::Update, DbEvent::Remove { uri: uri.into() });

        Some(())
    }
}
