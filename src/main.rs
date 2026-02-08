// TODO: remove before release
#![allow(unused_imports)]

mod db;
mod net;

mod local_controller;
mod main_controller;

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::json;
use std::fs::File;
use std::{path::PathBuf, thread::sleep, time::Duration};
use tokio::sync::mpsc as tokio_chan;
use tokio_util::sync::CancellationToken;

use crate::db::core::{Db, SharedDb};
use crate::db::db_controller;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO: cli options / config file
    tracing_subscriber::fmt()
        .with_writer(File::create("/tmp/foksal.log")?)
        .with_ansi(false)
        .init();

    let music_prefix = PathBuf::from("/home/antek/Main/music");
    let ignore_glob_set = GlobSet::empty();
    let allowed_exts = ["mp3".to_string(), "m4a".to_string(), "flac".to_string()];
    let db = Db::new(&music_prefix, &ignore_glob_set, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_prefix, ignore_glob_set, &allowed_exts)?;

    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    let main_controller_task = main_controller::spawn(2137, tx_raw_request, c_token.clone());

    // here choose local/headless/proxy depending on the cli args
    // let specific_controller_task = match { ... };
    let specific_controller_task = local_controller::spawn(db, rx_raw_request, c_token.clone());

    tokio::time::sleep(Duration::from_mins(1)).await;
    c_token.cancel();
    let _ = tokio::join!(main_controller_task, specific_controller_task);

    Ok(())
}
