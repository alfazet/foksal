use anyhow::{Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{self, Message as WsMessage},
};
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
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    mut rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
) -> Result<()> {
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let (tx_cancel, mut rx_cancel) = oneshot::channel();

    tokio::spawn(async move {
        loop {
            if let Some(msg) = rx_msg.recv().await {
                let _ = ws_write.send(msg).await;
            }
            if rx_cancel.try_recv().is_ok() {
                break;
            }
        }
    });

    while let Some(raw_request) = rx_raw_request.recv().await {
        let RawRequest { data, respond_to } = raw_request;
        let _ = tx_msg.send(WsMessage::Binary(data));
        let response_msg = ws_read
            .next()
            .await
            .ok_or(anyhow!("connection to headless instance interrupted"))??;
        match response_msg {
            WsMessage::Binary(response_bytes) => {
                let _ = respond_to.send(response_bytes);
            }
            // TODO: add a `ping` request that will transmit a ping and expect a ping back
            _ => bail!("headless instance sent invalid data"),
        }
    }
    let _ = tx_cancel.send(());

    Ok(())
}

pub async fn start(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    // TODO: audio controller task (to handle volume changes and everything else that we don't
    // forward to the headless instance)
    let res = tokio::select! {
        res = run(ws_stream, rx_raw_request) => res,
        _ = c_token.cancelled() => Ok(()),
    };
    c_token.cancel();

    res
}

pub async fn connect_to_headless(
    host: String,
    port: u16,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(format!("ws://{}:{}", host, port)).await?;

    Ok(ws_stream)
}
