use anyhow::Result;
use crossbeam::channel as cbeam_chan;
use globset::GlobSet;
use notify::{
    Config as WatcherConfig, Event as FsEvent, EventKind, PollWatcher, RecursiveMode,
    Result as NotifyResult, Watcher,
};
use std::{path::PathBuf, thread, time::Duration};

use crate::db::{core::SharedDb, fs_utils};

const POLL_COOLDOWN: u64 = 1; // in seconds

pub fn run(
    mut db: SharedDb,
    root: impl Into<PathBuf>,
    ignore_glob_set: GlobSet,
    allowed_exts: &[impl Into<String> + AsRef<str>],
) -> Result<()> {
    let root = root.into();
    let allowed_exts: Vec<String> = allowed_exts.iter().map(|s| s.as_ref().into()).collect();
    let (watcher_tx, watcher_rx) = cbeam_chan::unbounded::<NotifyResult<FsEvent>>();
    let watcher_config =
        WatcherConfig::default().with_poll_interval(Duration::from_secs(POLL_COOLDOWN));
    let mut watcher = PollWatcher::new(watcher_tx, watcher_config)?;

    let _ = thread::spawn(move || {
        let _ = watcher.watch(&root, RecursiveMode::Recursive);
        for event in watcher_rx.into_iter().flatten() {
            // react only to events regarding the relevant files
            if let Some(uri) = event.paths.first()
                && fs_utils::ext_matches(uri, &allowed_exts).is_some_and(|x| x)
                && !ignore_glob_set.is_match(uri)
            {
                match event.kind {
                    EventKind::Create(_) => {
                        println!("file `{:?}` was created", uri);
                        db.create(uri);
                    }
                    EventKind::Modify(_) => {
                        println!("file `{:?}` was modified", uri);
                        db.modify(uri);
                    }
                    EventKind::Remove(_) => {
                        println!("file `{:?}` was removed", uri);
                        db.remove(uri);
                    }
                    _ => (),
                }
            }
        }
    });

    Ok(())
}
