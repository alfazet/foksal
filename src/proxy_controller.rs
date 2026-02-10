use anyhow::{Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc as tokio_chan, oneshot},
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{self, Bytes, Message as WsMessage},
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

const TIMEOUT: u64 = 5; // in seconds

async fn run(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    mut rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
) -> Result<()> {
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let tx_msg_ping = tx_msg.clone();
    let (tx_response, mut rx_response) = tokio_chan::unbounded_channel();
    let c_token = CancellationToken::new();
    let (c_token_ping, c_token_pass_request, c_token_recv_request) =
        (c_token.clone(), c_token.clone(), c_token.clone());

    // task to check if the headless instance is alive
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = time::sleep(Duration::from_secs(TIMEOUT)) => (),
                _ = c_token_ping.cancelled() => break,
            };
            let _ = tx_msg_ping.send(WsMessage::Ping(Bytes::new()));
        }
    });

    // task to pass requests to the headless instance
    tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                msg = rx_msg.recv() => msg,
                _ = c_token_pass_request.cancelled() => break,
            };
            if let Some(msg) = msg {
                let _ = ws_write.send(msg).await;
            }
        }
    });

    // task to receive requests from local
    tokio::spawn(async move {
        loop {
            tokio::select! {
                raw_request = rx_raw_request.recv() => {
                    match raw_request {
                        Some(raw_request) => {
                            let RawRequest { data, respond_to } = raw_request;
                            let _ = tx_msg.send(WsMessage::Binary(data));
                            match rx_response.recv().await {
                                Some(bytes) => {
                                    let _ = respond_to.send(bytes);
                                }
                                _ => break,
                            }
                        }
                        None => break,
                    }
                }
                _ = c_token_recv_request.cancelled() => break,
            };
        }
    });

    let res = loop {
        tokio::select! {
            // timed out
            _ = time::sleep(Duration::from_secs(2 * TIMEOUT)) => {
                break Err(anyhow!("connection to headless timed out"));
            }
            // received a response from headless
            response_msg = ws_read.next() => {
                match response_msg {
                    Some(msg) => {
                        match msg {
                            Ok(WsMessage::Binary(bytes)) => {
                                let _ = tx_response.send(bytes);
                            }
                            Ok(WsMessage::Pong(_)) => (),
                            Ok(WsMessage::Close(_)) => break Ok(()),
                            _ => break Err(anyhow!("headless instance sent invalid data")),
                        }
                    }
                    None => break Err(anyhow!("connection to headless instance interrupted")),
                }
            }
        }
    };
    c_token.cancel();

    res
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
