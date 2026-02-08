use anyhow::{Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, event, info, instrument};

use crate::{
    db::{core::SharedDb, db_controller},
    net::{
        request::{DbRequest, ParsedRequest, RawRequest, RequestKind},
        response::Response,
    },
};

async fn run(
    mut rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    tx_db_request: tokio_chan::UnboundedSender<ParsedRequest<DbRequest>>,
) -> Result<()> {
    while let Some(raw_request) = rx_raw_request.recv().await {
        let request_kind: RequestKind =
            match serde_json::from_slice(raw_request.data()).map_err(|e| anyhow!(e)) {
                Ok(request_kind) => request_kind,
                Err(e) => {
                    let _ = raw_request
                        .respond_to
                        .send(Response::new_err(format!("unparseable request ({})", e)));
                    continue;
                }
            };
        let (parsed_request_respond_to, response_rx) = oneshot::channel();
        match request_kind {
            RequestKind::DbRequest(db_request) => {
                let parsed_request = ParsedRequest::new(db_request, parsed_request_respond_to);
                tx_db_request.send(parsed_request)?;
            }
        };
        let _ = raw_request.respond_to.send(response_rx.await?);
    }

    Ok(())
}

pub fn spawn(
    db: SharedDb,
    rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    c_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let (tx_db_request, rx_db_request) = tokio_chan::unbounded_channel();
        let db_controller_task = db_controller::spawn(db, rx_db_request, c_token.clone());
        let res = tokio::select! {
            res = run(rx_raw_request, tx_db_request) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        if let Err(e) = res {
            error!("local controller error ({})", e);
        }
        c_token.cancel();
        let _ = tokio::join!(db_controller_task);
    })
}
