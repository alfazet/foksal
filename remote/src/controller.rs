use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
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
use tracing::error;

use libfoksalcommon::net::{
    request::{FileRequest, RawDbRequest, RemoteRequest, SubscribeArgs, UnsubscribeArgs},
    response::{EventNotif, RemoteResponse, RemoteResponseInner, RemoteResponseKind, Response},
};
use libfoksaldb::{
    db_controller,
    request::{DbRequest, DbRequestKind},
};

use crate::config::ParsedRemoteConfig;

async fn handle_request(
    bytes: Bytes,
    tx_db_request: &tokio_chan::UnboundedSender<DbRequest>,
    tx_file_request: &tokio_chan::UnboundedSender<FileRequest>,
    tx_event: &tokio_chan::UnboundedSender<EventNotif>,
) -> Result<Option<RemoteResponseKind>> {
    let request_kind: RemoteRequest = match serde_json::from_slice(&bytes).map_err(|e| anyhow!(e)) {
        Ok(request_kind) => request_kind,
        Err(e) => {
            let response = Response::new_err(format!("invalid request ({})", e));
            let inner = RemoteResponseInner::Response(response);
            let response = RemoteResponse::new(inner, None);

            return Ok(Some(RemoteResponseKind::TextResponse(response)));
        }
    };

    match request_kind {
        RemoteRequest::DbRequest { request, client } => {
            let (respond_to, rx_response) = oneshot::channel();
            let request = match request {
                RawDbRequest::Subscribe(target) => {
                    let args = SubscribeArgs::new(target, client, tx_event.clone());
                    let kind = DbRequestKind::Subscribe(args);
                    DbRequest::new(kind, respond_to)
                }
                RawDbRequest::Unsubscribe(target) => {
                    let args = UnsubscribeArgs::new(target, client);
                    let kind = DbRequestKind::Unsubscribe(args);
                    DbRequest::new(kind, respond_to)
                }
                other_request => DbRequest::new(DbRequestKind::Raw(other_request), respond_to),
            };
            tx_db_request.send(request)?;
            let inner = RemoteResponseInner::Response(rx_response.await?);
            let response = RemoteResponse::new(inner, Some(client));

            Ok(Some(RemoteResponseKind::TextResponse(response)))
        }
        RemoteRequest::FileRequest(request) => {
            let (respond_to, rx_response) = oneshot::channel();
            let request = FileRequest::new(request, respond_to);
            tx_file_request.send(request)?;
            let bytes = rx_response.await?;

            Ok(Some(RemoteResponseKind::BinaryResponse(bytes.into())))
        }
    }
}

async fn handle_proxy(
    stream: TcpStream,
    tx_db_request: tokio_chan::UnboundedSender<DbRequest>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let (tx_event, mut rx_event) = tokio_chan::unbounded_channel::<EventNotif>();

    // task to respond to the proxy
    tokio::spawn(async move {
        while let Some(msg) = rx_msg.recv().await {
            let _ = ws_write.send(msg).await;
        }
    });

    // task to pass events to subscribing clients
    let tx_msg_clone = tx_msg.clone();
    tokio::spawn(async move {
        while let Some(notif) = rx_event.recv().await {
            let client = notif.subscriber;
            let notif = RemoteResponse::new(RemoteResponseInner::EventNotif(notif), Some(client));
            if let Ok(text) = notif.to_text() {
                let _ = tx_msg_clone.send(WsMessage::Text(text));
            }
        }
    });

    let res = loop {
        tokio::select! {
            msg = ws_read.next() => {
                match msg {
                    Some(msg) => match msg {
                        Ok(WsMessage::Binary(bytes)) => {
                            let response = handle_request(bytes, &tx_db_request, &tx_file_request, &tx_event).await?;
                            if let Some(response) = response {
                                let msg = match response {
                                    RemoteResponseKind::TextResponse(response) => {
                                        WsMessage::Text(response.to_text()?)
                                    }
                                    RemoteResponseKind::BinaryResponse(response) => {
                                        WsMessage::Binary(response.into())
                                    }
                                };
                                let _ = tx_msg.send(msg);
                            }
                        }
                        Ok(WsMessage::Ping(data)) => {
                            let _ = tx_msg.send(WsMessage::Pong(data));
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
        reason: Utf8Bytes::from_static("remote foksal instance shutting down"),
    })));

    res
}

async fn run(
    port: u16,
    tx_db_request: tokio_chan::UnboundedSender<DbRequest>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    c_token: CancellationToken,
) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    loop {
        tokio::select! {
            Ok((stream, _)) = listener.accept() => {
                let tx_db_request_clone = tx_db_request.clone();
                let tx_file_request_clone = tx_file_request.clone();
                let c_token_clone = c_token.clone();
                tokio::spawn(async move {
                    let res = handle_proxy(stream, tx_db_request_clone, tx_file_request_clone, c_token_clone).await;
                    if let Err(e) = res {
                        error!("proxy handler error ({})", e);
                    }
                });
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    }
}

pub fn spawn(config: ParsedRemoteConfig, c_token: CancellationToken) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let (tx_db_request, rx_db_request) = tokio_chan::unbounded_channel();
        let (tx_file_request, rx_file_request) = tokio_chan::unbounded_channel();

        let ParsedRemoteConfig {
            port,
            music_root,
            ignore_globset,
            allowed_exts,
        } = config;
        db_controller::spawn(
            music_root,
            ignore_globset,
            allowed_exts,
            rx_db_request,
            rx_file_request,
        )?;

        let res = tokio::select! {
            res = run(port, tx_db_request, tx_file_request, c_token.clone()) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        c_token.cancel();

        res
    })
}
