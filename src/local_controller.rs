use anyhow::{Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::{
    Bytes, Message as WsMessage, Utf8Bytes,
    protocol::{CloseFrame, frame::coding::CloseCode},
};
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, event, info, instrument, warn};

use crate::{
    db::{
        core::SharedDb,
        db_controller,
        request::{DbRequest, DbRequestKind},
    },
    net::{
        request::{
            LocalRequest, PlayerSubTarget, RawDbRequest, RawPlayerRequest, SubscribeArgs,
            UnsubscribeArgs,
        },
        response::{EventNotif, Response},
    },
    player::{
        core::{Player, PlayerEvent},
        player_controller,
        request::{PlayerRequest, PlayerRequestKind},
    },
};

async fn handle_request(
    bytes: Bytes,
    addr: &SocketAddr,
    tx_db_request: &tokio_chan::UnboundedSender<DbRequest>,
    tx_player_request: &tokio_chan::UnboundedSender<PlayerRequest>,
    tx_event: &tokio_chan::UnboundedSender<EventNotif>,
) -> Result<Response> {
    let request_kind: LocalRequest = match serde_json::from_slice(&bytes).map_err(|e| anyhow!(e)) {
        Ok(request_kind) => request_kind,
        Err(e) => return Ok(Response::new_err(format!("invalid request ({})", e))),
    };
    let (respond_to, rx_response) = oneshot::channel();

    match request_kind {
        LocalRequest::DbRequest(db_request) => {
            let request = match db_request {
                RawDbRequest::Subscribe(target) => {
                    let args = SubscribeArgs::new(target, *addr, tx_event.clone());
                    let kind = DbRequestKind::Subscribe(args);
                    DbRequest::new(kind, respond_to)
                }
                RawDbRequest::Unsubscribe(target) => {
                    let args = UnsubscribeArgs::new(target, *addr);
                    let kind = DbRequestKind::Unsubscribe(args);
                    DbRequest::new(kind, respond_to)
                }
                other_request => DbRequest::new(DbRequestKind::Raw(other_request), respond_to),
            };
            tx_db_request.send(request)?;
        }
        LocalRequest::PlayerRequest(player_request) => {
            let request = match player_request {
                RawPlayerRequest::Subscribe(target) => {
                    let args = SubscribeArgs::new(target, *addr, tx_event.clone());
                    let kind = PlayerRequestKind::Subscribe(args);
                    PlayerRequest::new(kind, respond_to)
                }
                RawPlayerRequest::Unsubscribe(target) => {
                    let args = UnsubscribeArgs::new(target, *addr);
                    let kind = PlayerRequestKind::Unsubscribe(args);
                    PlayerRequest::new(kind, respond_to)
                }
                other_request => {
                    PlayerRequest::new(PlayerRequestKind::Raw(other_request), respond_to)
                }
            };
            tx_player_request.send(request)?;
        }
    };

    Ok(rx_response.await?)
}

async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    tx_db_request: tokio_chan::UnboundedSender<DbRequest>,
    tx_player_request: tokio_chan::UnboundedSender<PlayerRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let (tx_event, mut rx_event) = tokio_chan::unbounded_channel::<EventNotif>();

    // task to respond to the client
    tokio::spawn(async move {
        while let Some(msg) = rx_msg.recv().await {
            let _ = ws_write.send(msg).await;
        }
    });

    // task to pass events to subscribing clients
    let tx_msg_clone = tx_msg.clone();
    tokio::spawn(async move {
        while let Some(notif) = rx_event.recv().await {
            if let Ok(bytes) = notif.to_bytes_local() {
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
                            let response = handle_request(bytes, &addr, &tx_db_request, &tx_player_request, &tx_event).await?.to_bytes()?;
                            let _ = tx_msg.send(WsMessage::Binary(response));
                        }
                        Ok(WsMessage::Ping(data)) => {
                            let _ = tx_msg.send(WsMessage::Pong(data));
                        }
                        Ok(WsMessage::Close(_)) => {
                            info!("connection closed by the client");
                            break Ok(());
                        }
                        Err(e) => {
                            break Err(anyhow!(e));
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
    port: u16,
    tx_db_request: tokio_chan::UnboundedSender<DbRequest>,
    tx_player_request: tokio_chan::UnboundedSender<PlayerRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    loop {
        tokio::select! {
            Ok((stream, addr)) = listener.accept() => {
                let tx_db_request_clone = tx_db_request.clone();
                let tx_player_request_clone = tx_player_request.clone();
                let c_token_clone = c_token.clone();
                tokio::spawn(async move {
                    let res = handle_client(stream, addr, tx_db_request_clone, tx_player_request_clone, c_token_clone).await;
                    if let Err(e) = res {
                        error!("client handler error ({})", e);
                    }
                });
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    }
}

pub fn spawn(
    port: u16,
    db: SharedDb,
    player: Player,
    c_token: CancellationToken,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let (tx_db_request, rx_db_request) = tokio_chan::unbounded_channel();
        let (tx_player_request, rx_player_request) = tokio_chan::unbounded_channel();
        player_controller::spawn(player, rx_player_request);
        db_controller::spawn_blocking(db, rx_db_request);
        let res = tokio::select! {
            res = run(port, tx_db_request, tx_player_request, c_token.clone()) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        c_token.cancel();

        res
    })
}
