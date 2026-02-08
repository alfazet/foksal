use anyhow::{Result, anyhow};
use tokio::{
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument};

use crate::{
    db::core::SharedDb,
    net::{
        core::JsonObject,
        request::{DbRequest, ParsedRequest, RawRequest, RequestKind},
        response::Response,
    },
};

async fn run(
    db: SharedDb,
    mut rx_db_request: tokio_chan::UnboundedReceiver<ParsedRequest<DbRequest>>,
) -> Result<()> {
    while let Some(db_request) = rx_db_request.recv().await {
        let response = match db_request.request {
            DbRequest::Metadata(args) => db.metadata(args),
        };
        let _ = db_request.respond_to.send(response);
    }

    Ok(())
}

pub fn spawn(
    db: SharedDb,
    rx_db_request: tokio_chan::UnboundedReceiver<ParsedRequest<DbRequest>>,
    c_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let res = tokio::select! {
            res = run(db, rx_db_request) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        if let Err(e) = res {
            error!("db controller error ({})", e);
        }
        c_token.cancel();
    })
}
