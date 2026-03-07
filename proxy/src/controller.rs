use anyhow::{Result, anyhow};
use crossbeam_channel as cbeam_chan;
use futures_util::{SinkExt, StreamExt};
use libfoksalaudio::{
    player_controller,
    request::{PlayerRequest, PlayerRequestKind},
    sink::{self, SinkError},
};
use libfoksalcommon::net::{
    request::{
        FileRequest, LocalRequest, RawPlayerRequest, RemoteRequest, SubscribeArgs, UnsubscribeArgs,
    },
    response::{EventNotif, RemoteResponse, RemoteResponseInner, Response},
};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc as tokio_chan, oneshot},
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{
        Message as WsMessage, Utf8Bytes,
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use crate::config::ParsedProxyConfig;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type ClientsMap = HashMap<SocketAddr, tokio_chan::UnboundedSender<RemoteResponseInner>>;

const PING_TIMEOUT: u64 = 10; // in seconds

async fn handle_client(
    tcp_stream: TcpStream,
    addr: SocketAddr,
    tx_player_request: tokio_chan::UnboundedSender<PlayerRequest>,
    tx_remote_request: tokio_chan::UnboundedSender<RemoteRequest>,
    mut rx_remote_response: tokio_chan::UnboundedReceiver<RemoteResponseInner>,
    mut rx_async_error: broadcast::Receiver<SinkError>,
    c_token: CancellationToken,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(tcp_stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let (tx_event, mut rx_event) = tokio_chan::unbounded_channel::<EventNotif>();

    // task to respond to the client
    tokio::spawn(async move {
        let version_msg = Response::version().to_bytes().unwrap();
        let _ = ws_write.send(WsMessage::Binary(version_msg)).await;
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

    // task to send errors to the client
    let tx_msg_clone = tx_msg.clone();
    tokio::spawn(async move {
        while let Ok(error) = rx_async_error.recv().await {
            if let Ok(bytes) = error.to_bytes() {
                let _ = tx_msg_clone.send(WsMessage::Binary(bytes));
            }
        }
    });

    let res = loop {
        tokio::select! {
            msg = ws_read.next() => {
                match msg {
                    Some(msg) => {
                        if let Ok(WsMessage::Binary(bytes)) = msg {
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
    ws_stream: WsStream,
    local_port: u16,
    tx_player_request: tokio_chan::UnboundedSender<PlayerRequest>,
    mut rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>,
    rx_async_error: broadcast::Receiver<SinkError>,
    c_token: CancellationToken,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port)).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_remote_request, mut rx_remote_request) =
        tokio_chan::unbounded_channel::<RemoteRequest>();
    let (tx_ping, mut rx_ping) = tokio_chan::unbounded_channel();
    let clients = Arc::new(RwLock::new(ClientsMap::new()));
    let clients_clone = Arc::clone(&clients);
    let rxs_file_response = Arc::new(Mutex::new(VecDeque::new()));
    let rxs_file_response_clone = Arc::clone(&rxs_file_response);
    let c_token_clone = c_token.clone();

    // task to ping the remote
    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(PING_TIMEOUT)).await;
            let _ = tx_ping.send(());
        }
    });

    // task to pass requests to the proxy->remote ws connection
    tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                Some(request) = rx_remote_request.recv() => {
                    WsMessage::Binary(request.to_bytes().unwrap())
                }
                Some(FileRequest { raw, respond_to }) = rx_file_request.recv() => {
                    let remote_request = RemoteRequest::FileRequest(raw);
                    rxs_file_response.lock().unwrap().push_back(respond_to);
                    WsMessage::Binary(remote_request.to_bytes().unwrap())
                }
                Some(_) = rx_ping.recv() => {
                    WsMessage::Ping("".into())
                }
            };
            let _ = ws_write.send(msg).await;
        }
    });

    // task to read responses from the remote and pass them to recipient clients
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(msg) = ws_read.next() => {
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
                            let respond_to = rxs_file_response_clone.lock().unwrap().pop_front();
                            match respond_to {
                                Some(tx) => {
                                    let _ = tx.send(bytes);
                                }
                                None => warn!("remote sent back an unprompted response"),
                            }
                        }
                        _ => (),
                    }
                }
                _ = time::sleep(Duration::from_secs(2 * PING_TIMEOUT)) => {
                    warn!("connection to remote instance timed out");
                    c_token_clone.cancel();
                }
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
                let rx_async_error_clone = rx_async_error.resubscribe();
                let c_token_clone = c_token.clone();
                let clients_clone = Arc::clone(&clients);
                tokio::spawn(async move {
                    let res = handle_client(tcp_stream, addr, tx_player_request_clone, tx_remote_request_clone, rx_remote_response, rx_async_error_clone, c_token_clone).await;
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
    ws_stream: WsStream,
    config: ParsedProxyConfig,
    c_token: CancellationToken,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let (tx_player_request, rx_player_request) = tokio_chan::unbounded_channel();
        let (tx_file_request, rx_file_request) = tokio_chan::unbounded_channel();
        let (tx_sink_response, rx_sink_response) = tokio_chan::unbounded_channel();
        let (tx_sink_request, rx_sink_request) = cbeam_chan::unbounded();
        let (tx_async_error, rx_async_error) = broadcast::channel(1);

        let ParsedProxyConfig {
            local_port: port,
            audio_backend,
            ..
        } = config;
        player_controller::spawn(tx_sink_request, rx_player_request, rx_sink_response);
        sink::spawn_blocking(
            audio_backend,
            tx_file_request,
            rx_sink_request,
            tx_sink_response,
            tx_async_error,
        )?;

        let res = tokio::select! {
            res = run(ws_stream, port, tx_player_request, rx_file_request, rx_async_error, c_token.clone()) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        c_token.cancel();

        res
    })
}

pub async fn connect_to_remote(host: impl AsRef<str>, port: u16) -> Result<WsStream> {
    let (ws_stream, _) =
        tokio_tungstenite::connect_async(format!("ws://{}:{}", host.as_ref(), port)).await?;

    Ok(ws_stream)
}
