use anyhow::Result;
use globset::GlobSet;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::db::{fs_utils, fs_watcher, song_metadata::SongMetadata};

type Table = BTreeMap<PathBuf, SongMetadata>;

/// TODO:
/// - keep m3u playlists
/// - allow relative paths in requests
#[derive(Debug, Default)]
pub struct Db {
    pub table: Table,
    music_root: PathBuf,
}

pub struct SharedDb {
    pub inner: Arc<RwLock<Db>>,
    pub music_root: PathBuf,
}

impl Db {
    pub fn new(
        music_root: impl AsRef<Path> + Into<PathBuf>,
        ignore_glob_set: &GlobSet,
        allowed_exts: &[impl AsRef<str>],
    ) -> Result<Self> {
        let uris = fs_utils::walk_dir(&music_root, ignore_glob_set.clone(), allowed_exts)?;
        let table = Self::init_table(uris);
        let music_root = music_root.as_ref().to_path_buf();

        Ok(Self { table, music_root })
    }

    fn create(&mut self, uri: impl AsRef<Path> + Into<PathBuf>, data: SongMetadata) {
        self.table.insert(uri.into(), data);
    }

    fn modify(&mut self, uri: impl AsRef<Path>, new_data: SongMetadata) {
        if let Some(data) = self.table.get_mut(uri.as_ref()) {
            *data = new_data;
        }
    }

    fn remove(&mut self, uri: impl AsRef<Path>) {
        self.table.remove(uri.as_ref());
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
        ignore_glob_set: GlobSet,
        allowed_exts: Vec<String>,
    ) -> Result<()> {
        fs_watcher::run(self.clone(), music_root, ignore_glob_set, allowed_exts)
    }

    pub fn create(&mut self, uri: impl AsRef<Path> + Into<PathBuf>) -> Result<()> {
        let data = SongMetadata::try_new(&uri)?;
        let mut db = self.inner.write().unwrap();
        db.create(uri, data);

        Ok(())
    }

    pub fn modify(&mut self, uri: impl AsRef<Path>) -> Result<()> {
        let data = SongMetadata::try_new(&uri)?;
        let mut db = self.inner.write().unwrap();
        db.modify(uri, data);

        Ok(())
    }

    pub fn remove(&mut self, uri: impl AsRef<Path>) {
        let mut db = self.inner.write().unwrap();
        db.remove(uri);
    }
}
