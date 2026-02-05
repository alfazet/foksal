use anyhow::Result;
use crossbeam::channel as cbeam_chan;
use globset::GlobSet;
use notify::{
    Config as WatcherConfig, Event as FsEvent, EventKind, PollWatcher, RecursiveMode,
    Result as NotifyResult, Watcher,
};
use std::{thread, time::Duration};

use crate::db::{core::SharedDb, fs_utils};

const POLL_COOLDOWN: u64 = 1; // in seconds

pub struct FsWatcher {
    db: SharedDb,
}

impl FsWatcher {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn run(&self, ignore_glob_set: GlobSet, allowed_exts: Vec<String>) -> Result<()> {
        let root = { self.db.0.read().unwrap().prefix.clone() };
        let (watcher_tx, watcher_rx) = cbeam_chan::unbounded::<NotifyResult<FsEvent>>();
        let watcher_config =
            WatcherConfig::default().with_poll_interval(Duration::from_secs(POLL_COOLDOWN));
        let mut watcher = PollWatcher::new(watcher_tx, watcher_config)?;
        let _ = thread::spawn(move || {
            let _ = watcher.watch(&root, RecursiveMode::Recursive);
            for event in watcher_rx.into_iter().flatten() {
                // react only to events regarding the relevant files
                if let Some(path) = event.paths.first()
                    && fs_utils::ext_matches(path, &allowed_exts).is_some_and(|x| x)
                    && !ignore_glob_set.is_match(path)
                {
                    match event.kind {
                        EventKind::Create(_) => {
                            println!("file `{:?}` was created", path);
                        }
                        EventKind::Modify(_) => {
                            println!("file `{:?}` was modified", path);
                        }
                        EventKind::Remove(_) => {
                            println!("file `{:?}` was removed", path);
                        }
                        _ => (),
                    }
                }
            }
        });

        Ok(())
    }
}
