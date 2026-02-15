// TODO: remove before release
#![allow(unused_imports)]

mod config;
mod db;
mod net;
mod player;

mod local_controller;
mod proxy_controller;
mod remote_controller;

use anyhow::{Result, bail};
use clap::Parser;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use tokio::{signal, sync::mpsc as tokio_chan, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::{
        CliArgs, CliConfig, LocalArgs, LocalConfig, Mode, ProxyArgs, ProxyConfig, RemoteArgs,
        RemoteConfig,
    },
    db::{
        core::{Db, SharedDb},
        db_controller,
    },
    player::core::Player,
};

fn init_db(
    music_root: impl AsRef<Path> + Into<PathBuf>,
    ignore_glob_set: GlobSet,
    allowed_exts: Vec<String>,
) -> Result<SharedDb> {
    let db = Db::new(music_root.as_ref(), &ignore_glob_set, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(music_root.as_ref(), ignore_glob_set, allowed_exts)?;

    Ok(db)
}

fn init_player() -> Player {
    Player::new()
}

async fn local_main(args: LocalArgs) -> Result<()> {
    let config = LocalConfig::merge_with_cli(args);
    let LocalConfig {
        local_port,
        music_root,
        ignore_glob_set,
        allowed_exts,
    } = config;
    let db = init_db(&music_root, ignore_glob_set, allowed_exts)?;
    let player = init_player();
    // TODO: init the decoder (should be part of DB)

    let c_token = CancellationToken::new();
    let local_controller = local_controller::spawn(local_port, db, player, c_token.clone());
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    local_controller.await?
}

async fn remote_main(args: RemoteArgs) -> Result<()> {
    let config = RemoteConfig::merge_with_cli(args);
    let RemoteConfig {
        local_port,
        music_root,
        ignore_glob_set,
        allowed_exts,
    } = config;
    let db = init_db(&music_root, ignore_glob_set, allowed_exts)?;

    let c_token = CancellationToken::new();
    let remote_controller = remote_controller::spawn(local_port, db, c_token.clone());
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    remote_controller.await?
}

async fn proxy_main(args: ProxyArgs) -> Result<()> {
    let config = ProxyConfig::merge_with_cli(args);
    let ProxyConfig {
        remote_addr,
        remote_port,
        local_port,
    } = config;
    let ws_stream = proxy_controller::connect_to_remote(remote_addr, remote_port).await?;
    let player = init_player();

    let c_token = CancellationToken::new();
    let proxy_controller = proxy_controller::spawn(ws_stream, local_port, player, c_token.clone());
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    proxy_controller.await?
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    let cli_config = CliConfig::merge_with_cli(cli_args);
    let CliConfig { mode, log_file } = cli_config;
    tracing_subscriber::fmt()
        .with_writer(File::create(log_file)?)
        .with_ansi(false)
        .with_line_number(true)
        .init();

    match mode {
        Mode::Local(args) => local_main(args).await,
        Mode::Remote(args) => remote_main(args).await,
        Mode::Proxy(args) => proxy_main(args).await,
    }
}
