use anyhow::{Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::{
    Message as WsMessage, Utf8Bytes,
    protocol::{CloseFrame, frame::coding::CloseCode},
};
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, event, info, instrument};

use crate::net::{request::RawRequest, response::Response};

async fn handle_client(
    stream: TcpStream,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (msg_tx, mut msg_rx) = tokio_chan::unbounded_channel();
    let (writer_cancel_tx, mut writer_cancel_rx) = oneshot::channel();

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
        tokio::select! {
            msg = ws_read.next() => {
                match msg {
                    Some(msg) => match msg {
                        Ok(WsMessage::Binary(bytes)) => {
                            let (respond_to, response_rx) = oneshot::channel();
                            let raw_request = RawRequest::new(bytes, respond_to);
                            tx_raw_request.send(raw_request)?;
                            let response = response_rx.await?;
                            let _ = msg_tx.send(WsMessage::Binary(response));
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
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    };
    let _ = msg_tx.send(WsMessage::Close(Some(CloseFrame {
        code: CloseCode::Normal,
        reason: Utf8Bytes::from_static("foksal closed"),
    })));
    let _ = writer_cancel_tx.send(());

    res
}

async fn run(
    port: u16,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    loop {
        tokio::select! {
            Ok((stream, _)) = listener.accept() => {
                let tx_raw_request_clone = tx_raw_request.clone();
                let c_token_clone = c_token.clone();
                tokio::spawn(async move {
                    let res = handle_client(stream, tx_raw_request_clone, c_token_clone).await;
                    if let Err(e) = res {
                        error!("client handler error ({})", e);
                    }
                });
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    }
}

pub async fn start(
    port: u16,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let res = run(port, tx_raw_request, c_token.clone()).await;
    c_token.cancel();

    res
}
