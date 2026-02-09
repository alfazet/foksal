// TODO: remove before release
#![allow(unused_imports)]

mod config;
mod db;
mod net;

mod headless_controller;
mod local_controller;
mod main_controller;
mod proxy_controller;

use anyhow::{Result, bail};
use clap::Parser;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs::File;
use tokio::{signal, sync::mpsc as tokio_chan, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    config::{
        CliArgs, CliConfig, HeadlessArgs, HeadlessConfig, LocalArgs, LocalConfig, Mode, ProxyArgs,
        ProxyConfig,
    },
    db::{
        core::{Db, SharedDb},
        db_controller,
    },
};

async fn local_main(args: LocalArgs) -> Result<()> {
    let config = LocalConfig::merge_with_cli(args);
    let LocalConfig {
        port,
        music_root,
        ignore_glob_set,
        allowed_exts,
    } = config;
    let db = Db::new(&music_root, &ignore_glob_set, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_root, ignore_glob_set, &allowed_exts)?;

    let mut task_set = JoinSet::new();
    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    task_set.spawn(main_controller::start(
        port,
        tx_raw_request,
        c_token.clone(),
    ));
    task_set.spawn(local_controller::start(db, rx_raw_request, c_token.clone()));
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    let mut overall_res = Ok(());
    while let Some(res) = task_set.join_next().await {
        if let Err(e) = res {
            error!("controller error ({})", e);
            overall_res = Err(e);
        }
    }

    Ok(overall_res?)
}

async fn proxy_main(args: ProxyArgs) -> Result<()> {
    let config = ProxyConfig::merge_with_cli(args);
    let ProxyConfig {
        headless_addr,
        headless_port,
        local_port,
    } = config;
    let ws_stream = proxy_controller::connect_to_headless(headless_addr, headless_port).await?;

    let mut task_set = JoinSet::new();
    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    task_set.spawn(main_controller::start(
        local_port,
        tx_raw_request,
        c_token.clone(),
    ));
    task_set.spawn(proxy_controller::start(
        ws_stream,
        rx_raw_request,
        c_token.clone(),
    ));
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    let mut overall_res = Ok(());
    while let Some(res) = task_set.join_next().await {
        if let Err(e) = res {
            error!("controller error ({})", e);
            overall_res = Err(e);
        }
    }

    Ok(overall_res?)
}

async fn headless_main(args: HeadlessArgs) -> Result<()> {
    let config = HeadlessConfig::merge_with_cli(args);
    let HeadlessConfig {
        port,
        music_root,
        ignore_glob_set,
        allowed_exts,
    } = config;
    let db = Db::new(&music_root, &ignore_glob_set, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_root, ignore_glob_set, &allowed_exts)?;

    let mut task_set = JoinSet::new();
    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    task_set.spawn(main_controller::start(
        port,
        tx_raw_request,
        c_token.clone(),
    ));
    task_set.spawn(headless_controller::start(
        db,
        rx_raw_request,
        c_token.clone(),
    ));
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    let mut overall_res = Ok(());
    while let Some(res) = task_set.join_next().await {
        if let Err(e) = res {
            error!("controller error ({})", e);
            overall_res = Err(e);
        }
    }

    Ok(overall_res?)
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
        Mode::Proxy(args) => proxy_main(args).await,
        Mode::Headless(args) => headless_main(args).await,
    }
}
