use anyhow::{Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{
        Bytes, Message as WsMessage, Utf8Bytes,
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, event, info, instrument, warn};

use crate::{
    net::{
        request::{LocalRequest, RawPlayerRequest, RemoteRequest, SubscribeArgs, UnsubscribeArgs},
        response::{EventNotif, RemoteResponse, RemoteResponseInner, RemoteResponseKind, Response},
    },
    player::{
        core::Player,
        player_controller,
        request::{PlayerRequest, PlayerRequestKind},
    },
};

async fn handle_client(
    tcp_stream: TcpStream,
    addr: SocketAddr,
    tx_player_request: tokio_chan::UnboundedSender<PlayerRequest>,
    tx_remote_request: tokio_chan::UnboundedSender<RemoteRequest>,
    mut rx_remote_response: tokio_chan::UnboundedReceiver<RemoteResponseInner>,
    c_token: CancellationToken,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(tcp_stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let (tx_event, mut rx_event) = tokio_chan::unbounded_channel::<EventNotif>();

    // task to respond to the client
    tokio::spawn(async move {
        while let Some(msg) = rx_msg.recv().await {
            let _ = ws_write.send(msg).await;
        }
    });

    // task to receive from rx_remote_response and send to tx_msg (and then to the client)
    let tx_msg_clone = tx_msg.clone();
    tokio::spawn(async move {
        while let Some(response) = rx_remote_response.recv().await {
            let bytes = match response {
                RemoteResponseInner::Response(response) => response.to_bytes(),
                RemoteResponseInner::EventNotif(notif) => notif.to_bytes(),
            }
            .unwrap();
            let _ = tx_msg_clone.send(WsMessage::Binary(bytes));
        }
    });

    // task to pass (local) events to subscribing clients
    let tx_msg_clone = tx_msg.clone();
    tokio::spawn(async move {
        while let Some(notif) = rx_event.recv().await {
            if let Ok(bytes) = notif.to_bytes() {
                let _ = tx_msg_clone.send(WsMessage::Binary(bytes));
            }
        }
    });

    let res = loop {
        tokio::select! {
            msg = ws_read.next() => {
                match msg {
                    Some(msg) => match msg {
                        Ok(WsMessage::Binary(bytes)) => {
                            let request_kind: LocalRequest = match serde_json::from_slice(&bytes).map_err(|e| anyhow!(e)) {
                                Ok(request_kind) => request_kind,
                                Err(e) => {
                                    let response = Response::new_err(format!("invalid request ({})", e));
                                    let _ = tx_msg.send(WsMessage::Binary(response.to_bytes()?));
                                    continue;
                                }
                            };
                            match request_kind {
                                LocalRequest::DbRequest(db_request) => {
                                    let remote_request = RemoteRequest::DbRequest { request: db_request, client: addr };
                                    let _ = tx_remote_request.send(remote_request);
                                }
                                LocalRequest::PlayerRequest(player_request) => {
                                    let (respond_to, rx_response) = oneshot::channel();
                                    let request = match player_request {
                                        RawPlayerRequest::Subscribe(target) => {
                                            let args = SubscribeArgs::new(target, addr, tx_event.clone());
                                            let kind = PlayerRequestKind::Subscribe(args);
                                            PlayerRequest::new(kind, respond_to)
                                        }
                                        RawPlayerRequest::Unsubscribe(target) => {
                                            let args = UnsubscribeArgs::new(target, addr);
                                            let kind = PlayerRequestKind::Unsubscribe(args);
                                            PlayerRequest::new(kind, respond_to)
                                        }
                                        other_request => {
                                            PlayerRequest::new(PlayerRequestKind::Raw(other_request), respond_to)
                                        }
                                    };
                                    tx_player_request.send(request)?;
                                    let response = rx_response.await?.to_bytes()?;
                                    let _ = tx_msg.send(WsMessage::Binary(response));
                                }
                            }
                        }
                        _ => (),
                    }
                    None => break Err(anyhow!("connection closed unexpectedly")),
                }
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    };
    let _ = tx_msg.send(WsMessage::Close(Some(CloseFrame {
        code: CloseCode::Normal,
        reason: Utf8Bytes::from_static("foksal shutting down"),
    })));

    res
}

async fn run(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    local_port: u16,
    tx_player_request: tokio_chan::UnboundedSender<PlayerRequest>,
    tx_remote_request: tokio_chan::UnboundedSender<RemoteRequest>,
    mut rx_remote_request: tokio_chan::UnboundedReceiver<RemoteRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port)).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let clients = Arc::new(RwLock::new(HashMap::<
        SocketAddr,
        tokio_chan::UnboundedSender<RemoteResponseInner>,
    >::new()));
    let clients_clone = Arc::clone(&clients);

    // task to pass requests to the proxy->remote ws connection
    tokio::spawn(async move {
        // TODO: this need to also receive file requests
        while let Some(request) = rx_remote_request.recv().await {
            let _ = ws_write
                .send(WsMessage::Binary(request.to_bytes().unwrap()))
                .await;
        }
    });

    // task to read responses from the remote and pass them to recipient clients
    tokio::spawn(async move {
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    let Ok(response) = serde_json::from_str::<RemoteResponse>(&text) else {
                        return;
                    };
                    if let Some(client) = response.client
                        && let Some(tx) = clients_clone.read().unwrap().get(&client)
                    {
                        let _ = tx.send(response.inner);
                    }
                }
                Ok(WsMessage::Binary(bytes)) => {
                    // pass this to the oneshow channel back to the sink
                }
                _ => (),
            }
        }
    });

    loop {
        tokio::select! {
            Ok((tcp_stream, addr)) = listener.accept() => {
                let (tx_remote_response, rx_remote_response) = tokio_chan::unbounded_channel();
                {
                    clients.write().unwrap().insert(addr, tx_remote_response);
                }
                let tx_player_request_clone = tx_player_request.clone();
                let tx_remote_request_clone = tx_remote_request.clone();
                let c_token_clone = c_token.clone();
                let clients_clone = Arc::clone(&clients);
                tokio::spawn(async move {
                    let res = handle_client(tcp_stream, addr, tx_player_request_clone, tx_remote_request_clone, rx_remote_response, c_token_clone).await;
                    if let Err(e) = res {
                        error!("client handler error ({})", e);
                    }
                    clients_clone.write().unwrap().remove(&addr);
                });
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    }
}

pub fn spawn(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    local_port: u16,
    player: Player,
    c_token: CancellationToken,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let (tx_player_request, rx_player_request) = tokio_chan::unbounded_channel();
        player_controller::spawn(player, rx_player_request);
        let (tx_remote_request, rx_remote_request) = tokio_chan::unbounded_channel();
        let res = tokio::select! {
            res = run(ws_stream, local_port, tx_player_request, tx_remote_request.clone(), rx_remote_request, c_token.clone()) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        c_token.cancel();

        res
    })
}

pub async fn connect_to_remote(
    host: String,
    port: u16,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(format!("ws://{}:{}", host, port)).await?;

    Ok(ws_stream)
}
