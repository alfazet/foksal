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

use crate::net::{request::RawRequest, response::Response};

async fn handle_client(
    stream: TcpStream,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (msg_tx, mut msg_rx) = tokio_chan::unbounded_channel();
    let (writer_cancel_tx, mut writer_cancel_rx) = oneshot::channel();

    // a task that responds to clients
    tokio::spawn(async move {
        loop {
            if let Some(msg) = msg_rx.recv().await {
                let _ = ws_write.send(msg).await;
            }
            if writer_cancel_rx.try_recv().is_ok() {
                break;
            }
        }
    });

    let res = loop {
        match ws_read.next().await {
            Some(msg) => match msg {
                Ok(WsMessage::Binary(bytes)) => {
                    let (respond_to, response_rx) = oneshot::channel();
                    let raw_request = RawRequest::new(bytes, respond_to);
                    tx_raw_request.send(raw_request)?;
                    let response = response_rx.await?;
                    let _ = msg_tx.send(WsMessage::Binary(response.to_bytes()?));
                }
                Ok(WsMessage::Ping(data)) => {
                    let _ = msg_tx.send(WsMessage::Pong(data));
                }
                Ok(WsMessage::Close(_)) => {
                    info!("connection closed by the client");
                    break Ok(());
                }
                Err(e) => {
                    break Err(anyhow!(e));
                }
                _ => {
                    let response = Response::usage();
                    let _ = msg_tx.send(WsMessage::Binary(response.to_bytes()?));
                }
            },
            None => break Err(anyhow!("connection closed unexpectedly")),
        }
    };
    let _ = writer_cancel_tx.send(());

    res
}

// TODO: pass full config
async fn run(port: u16, tx_raw_request: tokio_chan::UnboundedSender<RawRequest>) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    while let Ok((stream, _)) = listener.accept().await {
        let tx_raw_request = tx_raw_request.clone();
        tokio::spawn(async move {
            let res = handle_client(stream, tx_raw_request.clone()).await;
            if let Err(e) = res {
                error!("client handler error ({})", e);
            }
        });
    }

    Ok(())
}

pub fn spawn(
    port: u16,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    c_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let res = tokio::select! {
            res = run(port, tx_raw_request) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        if let Err(e) = res {
            error!("main controller error ({})", e);
        }
        c_token.cancel();
    })
}
