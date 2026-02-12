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
use uuid::Uuid;

use crate::{
    db::{core::SharedDb, db_controller},
    net::{
        request::{DbRequest, HeadlessRequestKind, IntraRequest, ParsedRequest, RawRequest},
        response::Response,
    },
};

async fn run(
    mut rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    mut rx_intra: tokio_chan::UnboundedReceiver<ParsedRequest<IntraRequest>>,
    tx_db_request: tokio_chan::UnboundedSender<ParsedRequest<DbRequest>>,
) -> Result<()> {
    // the headless controller can receive intra requests both locally and through the websocket
    // from a proxy
    tokio::spawn(async move {
        while let Some(intra_request) = rx_intra.recv().await {
            match intra_request.request {
                IntraRequest::Register => {
                    let id = Uuid::new_v4();
                    let _ = intra_request
                        .respond_to
                        .send(Response::new_ok().with_item("id", &id));
                }
            }
        }
    });

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
        let (parsed_request_respond_to, rx_response) = oneshot::channel();
        match request_kind {
            HeadlessRequestKind::IntraRequest(intra_request) => match intra_request {
                IntraRequest::Register => {
                    let id = Uuid::new_v4();
                    let _ = parsed_request_respond_to.send(Response::new_ok().with_item("id", &id));
                }
            },
            HeadlessRequestKind::DbRequest(db_request) => {
                let parsed_request = ParsedRequest::new(db_request, parsed_request_respond_to);
                tx_db_request.send(parsed_request)?;
            }
        };
        let response = rx_response.await?;
        let _ = raw_request.respond_to.send(response.to_bytes()?);
    }

    Ok(())
}

pub async fn start(
    db: SharedDb,
    rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    rx_intra: tokio_chan::UnboundedReceiver<ParsedRequest<IntraRequest>>,
    c_token: CancellationToken,
) -> Result<()> {
    let (tx_db_request, rx_db_request) = tokio_chan::unbounded_channel();
    db_controller::spawn_blocking(db, rx_db_request);
    let res = tokio::select! {
        res = run(rx_raw_request, rx_intra, tx_db_request) => res,
        _ = c_token.cancelled() => Ok(()),
    };
    c_token.cancel();

    res
}
