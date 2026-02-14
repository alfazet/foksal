use anyhow::Result;
use crossbeam::channel as cbeam_chan;
use globset::GlobSet;
use notify::{
    Config as WatcherConfig, Event as FsEvent, EventKind, PollWatcher, RecursiveMode,
    Result as NotifyResult, Watcher,
};
use std::{path::PathBuf, thread, time::Duration};
use tracing::warn;

use crate::db::{core::SharedDb, fs_utils};

const POLL_TIMEOUT: u64 = 2; // in seconds

pub fn run(
    mut db: SharedDb,
    root: impl Into<PathBuf>,
    ignore_glob_set: GlobSet,
    allowed_exts: Vec<String>,
) -> Result<()> {
    let root = root.into();
    let (tx_watcher, rx_watcher) = cbeam_chan::unbounded::<NotifyResult<FsEvent>>();
    let watcher_config =
        WatcherConfig::default().with_poll_interval(Duration::from_secs(POLL_TIMEOUT));
    let mut watcher = PollWatcher::new(tx_watcher, watcher_config)?;

    let _ = thread::spawn(move || {
        let _ = watcher.watch(&root, RecursiveMode::Recursive);
        for event in rx_watcher.into_iter().flatten() {
            if let Some(uri) = event.paths.first()
                && fs_utils::ext_matches(uri, &allowed_exts).is_some_and(|x| x)
                && !ignore_glob_set.is_match(uri)
            {
                match event.kind {
                    EventKind::Create(_) => {
                        if let Err(e) = db.create(uri) {
                            warn!("db `create` error ({})", e);
                        }
                    }
                    EventKind::Modify(_) => {
                        if let Err(e) = db.modify(uri) {
                            warn!("db `modify` error ({})", e);
                        }
                    }
                    EventKind::Remove(_) => {
                        if db.remove(uri).is_none() {
                            warn!(
                                "db `remove` error (file {} doesn't exist)",
                                uri.to_string_lossy()
                            );
                        }
                    }
                    _ => (),
                }
            }
        }
    });

    Ok(())
}
