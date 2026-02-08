// TODO: remove before release
#![allow(unused_imports)]

mod db;
mod net;

mod local_controller;
mod main_controller;
mod proxy_controller;

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::json;
use std::env;
use std::fs::File;
use std::{path::PathBuf, thread::sleep, time::Duration};
use tokio::{signal, sync::mpsc as tokio_chan};
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

    let (local_port, remote_port) = (2137, 2137);
    let headless_host = env::var("AZURE_IP").unwrap();
    let music_prefix = PathBuf::from("/home/antek/Main/music");
    let ignore_glob_set = GlobSet::empty();
    let allowed_exts = ["mp3".to_string(), "m4a".to_string(), "flac".to_string()];
    let db = Db::new(&music_prefix, &ignore_glob_set, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_prefix, ignore_glob_set, &allowed_exts)?;

    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    let main_controller_task = main_controller::spawn(2137, tx_raw_request, c_token.clone());

    let instance_kind = "proxy";
    let specific_controller_task = match instance_kind {
        "proxy" => {
            let ws_stream =
                proxy_controller::connect_to_headless(headless_host, remote_port).await?;
            proxy_controller::spawn(ws_stream, rx_raw_request, c_token.clone())
        }
        _ => local_controller::spawn(db, rx_raw_request, c_token.clone()),
    };

    signal::ctrl_c().await?;
    c_token.cancel();
    let _ = tokio::join!(main_controller_task, specific_controller_task);

    Ok(())
}
