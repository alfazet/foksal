// TODO: remove before release
#![allow(unused_imports)]

mod config;
mod db;
mod net;
mod player;

mod headless_controller;
mod local_controller;
mod main_controller;
mod proxy_controller;

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
        CliArgs, CliConfig, HeadlessArgs, HeadlessConfig, LocalArgs, LocalConfig, Mode, ProxyArgs,
        ProxyConfig,
    },
    db::{
        core::{Db, SharedDb},
        db_controller,
    },
    net::request::{IntraRequest, ParsedRequest, RawRequest},
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

fn init_player(music_root: impl Into<PathBuf>) -> Player {
    Player::new(music_root)
}

async fn common_main(
    local_port: u16,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    tx_intra: tokio_chan::UnboundedSender<ParsedRequest<IntraRequest>>,
    mut join_set: JoinSet<Result<()>>,
    c_token: CancellationToken,
) -> Result<()> {
    join_set.spawn(main_controller::start(
        local_port,
        tx_raw_request,
        tx_intra,
        c_token.clone(),
    ));

    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    let mut overall_res = Ok(());
    while let Some(res) = join_set.join_next().await {
        if let Ok(Err(e)) = res {
            error!("controller error ({})", e);
            overall_res = Err(e);
        }
    }

    overall_res
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
    let player = init_player(&music_root);

    let mut join_set = JoinSet::new();
    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    let (tx_intra, rx_intra) = tokio_chan::unbounded_channel();
    join_set.spawn(local_controller::start(
        db,
        player,
        rx_raw_request,
        rx_intra,
        c_token.clone(),
    ));

    common_main(local_port, tx_raw_request, tx_intra, join_set, c_token).await
}

async fn proxy_main(args: ProxyArgs) -> Result<()> {
    let config = ProxyConfig::merge_with_cli(args);
    let ProxyConfig {
        headless_addr,
        headless_port,
        local_port,
    } = config;
    let ws_stream = proxy_controller::connect_to_headless(headless_addr, headless_port).await?;

    let mut join_set = JoinSet::new();
    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    // looks absurd, but we need this to pass intra requests over to the headless instance
    let tx_raw_request_clone = tx_raw_request.clone();
    let (tx_intra, rx_intra) = tokio_chan::unbounded_channel();
    join_set.spawn(proxy_controller::start(
        ws_stream,
        rx_raw_request,
        rx_intra,
        tx_raw_request_clone,
        c_token.clone(),
    ));

    common_main(local_port, tx_raw_request, tx_intra, join_set, c_token).await
}

async fn headless_main(args: HeadlessArgs) -> Result<()> {
    let config = HeadlessConfig::merge_with_cli(args);
    let HeadlessConfig {
        local_port,
        music_root,
        ignore_glob_set,
        allowed_exts,
    } = config;
    let db = init_db(music_root, ignore_glob_set, allowed_exts)?;

    let mut join_set = JoinSet::new();
    let c_token = CancellationToken::new();
    let (tx_raw_request, rx_raw_request) = tokio_chan::unbounded_channel();
    let (tx_intra, rx_intra) = tokio_chan::unbounded_channel();
    join_set.spawn(headless_controller::start(
        db,
        rx_raw_request,
        rx_intra,
        c_token.clone(),
    ));

    common_main(local_port, tx_raw_request, tx_intra, join_set, c_token).await
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
