mod config;
mod controller;

use anyhow::Result;
use clap::Parser;
use tokio::signal;
use tokio_util::sync::CancellationToken;

use crate::config::*;
use foksalcommon::utils;

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = LocalArgs::parse();
    utils::setup_logging(cli_args.log_file.as_deref())?;
    let config = ParsedLocalConfig::try_new(cli_args)?;
    let c_token = CancellationToken::new();
    let controller = controller::spawn(config, c_token.clone());
    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = c_token.cancelled() => (),
    }
    c_token.cancel();

    controller.await?
}
