use anyhow::Result;
use globset::GlobSet;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::db::{fs_utils, fs_watcher};

type SongData = i32;

type Table = BTreeMap<PathBuf, SongData>;

/// TODO: keep m3u playlists
#[derive(Debug, Default)]
pub struct Db {
    music_prefix: PathBuf,
    table: Table,
}

pub struct SharedDb(pub Arc<RwLock<Db>>);

impl Db {
    pub fn new(
        music_prefix: impl AsRef<Path> + Into<PathBuf>,
        ignore_glob_set: &GlobSet,
        allowed_exts: &[impl AsRef<str>],
    ) -> Result<Self> {
        let stripped_uris =
            fs_utils::walk_dir(&music_prefix, ignore_glob_set.clone(), allowed_exts)?;
        let music_prefix = music_prefix.as_ref().to_path_buf();
        let table = Self::init_table(stripped_uris);

        Ok(Self {
            music_prefix,
            table,
        })
    }

    fn get(&self, uri: impl AsRef<Path>) -> Option<&SongData> {
        let uri = fs_utils::strip_if_absolute(&uri, &self.music_prefix)?;
        self.table.get(uri)
    }

    fn create(&mut self, uri: impl AsRef<Path>, data: SongData) {
        if let Some(uri) = fs_utils::strip_if_absolute(&uri, &self.music_prefix) {
            self.table.insert(uri.into(), data);
        }
    }

    fn modify(&mut self, uri: impl AsRef<Path>, new_data: SongData) {
        if let Some(uri) = fs_utils::strip_if_absolute(&uri, &self.music_prefix)
            && let Some(data) = self.table.get_mut(uri)
        {
            *data = new_data;
        }
    }

    fn remove(&mut self, uri: impl AsRef<Path>) {
        if let Some(uri) = fs_utils::strip_if_absolute(&uri, &self.music_prefix) {
            self.table.remove(uri);
        }
    }

    fn init_table(stripped_uris: impl IntoParallelIterator<Item = PathBuf>) -> Table {
        stripped_uris
            .into_par_iter()
            .filter_map(move |uri| Some((uri, 2137)))
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
        music_prefix: impl Into<PathBuf>,
        ignore_glob_set: GlobSet,
        allowed_exts: &[impl AsRef<str> + Into<String>],
    ) -> Result<()> {
        fs_watcher::run(self.clone(), music_prefix, ignore_glob_set, allowed_exts)
    }

    pub fn get(&self, uri: impl AsRef<Path>) -> Option<SongData> {
        let db = self.0.read().unwrap();
        db.get(uri.as_ref()).cloned()
    }

    pub fn create(&mut self, uri: impl AsRef<Path>) {
        // TODO: prepare the SongData struct before acquiring the lock
        let mut db = self.0.write().unwrap();
        db.create(uri, 7312);
    }

    pub fn modify(&mut self, uri: impl AsRef<Path>) {
        // TODO: fetch new song metadata from the uri
        let mut db = self.0.write().unwrap();
        db.modify(uri, 1234);
    }

    pub fn remove(&mut self, uri: impl AsRef<Path>) {
        let mut db = self.0.write().unwrap();
        db.remove(uri);
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
        assert_eq!(db.table.len(), 2);

        let mut iter = db.table.iter();
        let r1 = iter.next().unwrap();
        let r2 = iter.next().unwrap();
        assert_eq!(r1.0, Path::new("song1.mp3"));
        assert_eq!(r2.0, Path::new("song2.flac"));
    }

    #[test]
    fn test_crud() {
        let mut db = Db::default();

        db.create("abc", 1);
        db.create("def", 2);
        db.create("xyz", 10);
        assert_eq!(db.table.len(), 3);
        assert_eq!(db.get("abc"), Some(&1));
        assert_eq!(db.get("uvw"), None);

        db.modify("def", 22);
        assert_eq!(db.get("def"), Some(&22));

        db.remove("def");
        assert_eq!(db.get("def"), None);
    }
}
