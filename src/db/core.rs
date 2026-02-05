use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    cmp::Ordering,
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::db::{
    fs_utils,
    fs_watcher::{self, FsWatcher},
};

/// TODO: song metadata
#[derive(Debug)]
struct Row {
    uri: PathBuf, // relative to the db prefix
}

/// TODO: keep m3u playlists
#[derive(Debug)]
pub struct Db {
    pub prefix: PathBuf,
    rows: BTreeSet<Row>,
}

pub struct SharedDb(pub Arc<RwLock<Db>>);

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.uri.eq(&other.uri)
    }
}

impl Eq for Row {}

impl PartialOrd for Row {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Row {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uri.cmp(&other.uri)
    }
}

impl Row {
    pub fn new(uri: impl Into<PathBuf>) -> Self {
        Self { uri: uri.into() }
    }
}

impl Db {
    pub fn new(
        music_prefix: impl AsRef<Path> + Into<PathBuf>,
        ignore_glob_set: &GlobSet,
        allowed_exts: &[impl AsRef<str>],
    ) -> Result<Self> {
        let stripped_uris =
            fs_utils::walk_dir(&music_prefix, ignore_glob_set.clone(), allowed_exts)?;
        let music_prefix = music_prefix.as_ref().to_path_buf();
        let rows = Self::into_rows(stripped_uris);

        Ok(Self {
            prefix: music_prefix,
            rows,
        })
    }

    fn into_rows(stripped_uris: impl IntoParallelIterator<Item = PathBuf>) -> BTreeSet<Row> {
        stripped_uris
            .into_par_iter()
            .filter_map(move |uri| Some(Row::new(uri)))
            .collect()
    }
}

impl Clone for SharedDb {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl SharedDb {
    pub fn new(db: Db) -> Self {
        Self(Arc::new(RwLock::new(db)))
    }

    /// starts the watcher daemon on a separate thread
    pub fn start_fs_watcher(
        &self,
        ignore_glob_set: GlobSet,
        allowed_exts: Vec<String>,
    ) -> Result<()> {
        let watcher = FsWatcher::new(self.clone());
        watcher.run(ignore_glob_set, allowed_exts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_db_new() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();
        File::create(root.join("song1.mp3")).unwrap();
        File::create(root.join("song2.flac")).unwrap();
        File::create(root.join("ignored")).unwrap();
        let allowed_exts = ["mp3", "flac"];
        let db = Db::new(root, &GlobSet::default(), &allowed_exts).unwrap();
        assert_eq!(db.rows.len(), 2);

        let mut iter = db.rows.iter();
        let r1 = iter.next().unwrap();
        let r2 = iter.next().unwrap();
        assert_eq!(r1.uri, PathBuf::from("song1.mp3"));
        assert_eq!(r2.uri, PathBuf::from("song2.flac"));
    }
}
