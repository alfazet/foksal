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
        request::{DbRequest, HeadlessRequestKind, ParsedRequest, RawRequest},
        response::Response,
    },
};

async fn run(
    mut rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    tx_db_request: tokio_chan::UnboundedSender<ParsedRequest<DbRequest>>,
) -> Result<()> {
    while let Some(raw_request) = rx_raw_request.recv().await {
        let request_kind: HeadlessRequestKind =
            match serde_json::from_slice(raw_request.data()).map_err(|e| anyhow!(e)) {
                Ok(request_kind) => request_kind,
                Err(e) => {
                    let _ = raw_request.respond_to.send(
                        Response::new_err(format!("unparseable request ({})", e)).to_bytes()?,
                    );
                    continue;
                }
            };
        let (parsed_request_respond_to, response_rx) = oneshot::channel();
        match request_kind {
            HeadlessRequestKind::DbRequest(db_request) => {
                let parsed_request = ParsedRequest::new(db_request, parsed_request_respond_to);
                tx_db_request.send(parsed_request)?;
            }
        };
        let response = response_rx.await?;
        let _ = raw_request.respond_to.send(response.to_bytes()?);
    }

    Ok(())
}

pub async fn start(
    db: SharedDb,
    rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let (tx_db_request, rx_db_request) = tokio_chan::unbounded_channel();
    db_controller::spawn_blocking(db, rx_db_request);
    let res = tokio::select! {
        res = run(rx_raw_request, tx_db_request) => res,
        _ = c_token.cancelled() => Ok(()),
    };
    c_token.cancel();

    res
}
