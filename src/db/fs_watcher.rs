use anyhow::Result;
use crossbeam::channel as cbeam_chan;
use notify::{
    Config as WatcherConfig, Event as FsEvent, PollWatcher, RecursiveMode, Result as NotifyResult,
    Watcher,
};
use std::{thread, time::Duration};

use crate::db::core::SharedDb;

const POLL_COOLDOWN: u64 = 1; // in seconds

pub struct FsWatcher {
    db: SharedDb,
}

impl FsWatcher {
    pub fn new(db: SharedDb) -> Self {
        Self { db }
    }

    pub fn run(&self) -> Result<()> {
        let root = { self.db.0.read().unwrap().prefix.clone() };
        let (watcher_tx, watcher_rx) = cbeam_chan::unbounded::<NotifyResult<FsEvent>>();
        let watcher_config =
            WatcherConfig::default().with_poll_interval(Duration::from_secs(POLL_COOLDOWN));
        let mut watcher = PollWatcher::new(watcher_tx, watcher_config)?;
        let _ = thread::spawn(move || {
            let _ = watcher.watch(&root, RecursiveMode::Recursive);
            for event in watcher_rx.into_iter().flatten() {
                println!("{:?}", event);
            }
        });

        Ok(())
    }
}
