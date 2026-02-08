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
    let (tx_to_headless, mut rx_to_headless) = tokio_chan::unbounded_channel();
    let (tx_from_headless, mut rx_from_headless) = tokio_chan::unbounded_channel();
    let (tx_cancel, mut rx1_cancel) = broadcast::channel(1);
    let mut rx2_cancel = tx_cancel.subscribe();

    tokio::spawn(async move {
        loop {
            if let Some(msg) = rx_to_headless.recv().await {
                let _ = ws_write.send(msg).await;
            }
            if rx1_cancel.try_recv().is_ok() {
                break;
            }
        }
    });

    tokio::spawn(async move {
        loop {
            if let Some(msg) = ws_read.next().await {
                let _ = tx_from_headless.send(msg);
            }
            if rx2_cancel.try_recv().is_ok() {
                break;
            }
        }
    });

    while let Some(raw_request) = rx_raw_request.recv().await {
        let RawRequest { data, respond_to } = raw_request;
        let _ = tx_to_headless.send(WsMessage::Binary(data));
        let response_msg = rx_from_headless
            .recv()
            .await
            .ok_or(anyhow!("connection to headless instance interrupted"))??;
        match response_msg {
            WsMessage::Binary(response_bytes) => {
                let response = serde_json::from_slice(&response_bytes)?;
                let _ = respond_to.send(response);
            }
            _ => bail!("headless instance sent invalid data"),
        }
    }
    let _ = tx_cancel.send(());

    Ok(())
}

pub fn spawn(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    c_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // TODO: audio controller task (to handle volume changes and everything else that we don't
        // forward to the headless instance)
        let res = tokio::select! {
            res = run(ws_stream, rx_raw_request) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        if let Err(e) = res {
            error!("proxy controller error ({})", e);
        }
        c_token.cancel();
    })
}

pub async fn connect_to_headless(
    host: String,
    port: u16,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(format!("ws://{}:{}", host, port)).await?;

    Ok(ws_stream)
}
