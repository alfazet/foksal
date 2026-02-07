// TODO: remove before release
#![allow(unused_imports)]

mod db;
mod net;

mod main_controller;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::json;
use std::fs::File;
use std::{path::PathBuf, thread::sleep, time::Duration};
use tokio::sync::mpsc as tokio_chan;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    // TODO: cli options / config file
    tracing_subscriber::fmt()
        .with_writer(File::create("/tmp/foksal.log").unwrap())
        .with_ansi(false)
        .init();

    let music_prefix = PathBuf::from("/home/antek/Main/music");
    let ignore_glob_set = GlobSet::empty();
    let allowed_exts = ["mp3".to_string(), "m4a".to_string(), "flac".to_string()];
    // let db = Db::new(&music_prefix, &ignore_glob_set, &allowed_exts).unwrap();
    // let db = SharedDb::new(db);
    // db.start_fs_watcher(&music_prefix, ignore_glob_set, &allowed_exts)
    //     .unwrap();

    let c_token = CancellationToken::new();
    let (tx_raw_request, _) = tokio_chan::unbounded_channel();
    let _ = main_controller::spawn(2137, tx_raw_request, c_token.clone());

    tokio::time::sleep(Duration::from_mins(1)).await;
    c_token.cancel();
}
