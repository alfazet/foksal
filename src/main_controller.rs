use anyhow::{Result, anyhow, bail};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use std::net::SocketAddr;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        Bytes, Message as WsMessage, Utf8Bytes,
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, event, info, instrument};

use crate::net::{
    request::{IntraRequest, ParsedRequest, RawRequest},
    response::Response,
};

async fn assign_id(
    tx_msg: &tokio_chan::UnboundedSender<WsMessage>,
    tx_intra: &tokio_chan::UnboundedSender<ParsedRequest<IntraRequest>>,
) -> Result<()> {
    let (respond_to, rx_response) = oneshot::channel();
    let request = ParsedRequest::new(IntraRequest::Register, respond_to);
    tx_intra.send(request)?;
    let response = rx_response.await?;
    let _ = tx_msg.send(WsMessage::Binary(response.to_bytes()?));

    Ok(())
}

async fn handle_client(
    stream: TcpStream,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    tx_intra: tokio_chan::UnboundedSender<ParsedRequest<IntraRequest>>,
    c_token: CancellationToken,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx_msg, mut rx_msg) = tokio_chan::unbounded_channel();
    let (tx_writer_cancel, mut rx_writer_cancel) = oneshot::channel();
    assign_id(&tx_msg, &tx_intra).await?;

    tokio::spawn(async move {
        loop {
            if let Some(msg) = rx_msg.recv().await {
                let _ = ws_write.send(msg).await;
            }
            if rx_writer_cancel.try_recv().is_ok() {
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
                            let (respond_to, rx_response) = oneshot::channel();
                            let raw_request = RawRequest::new(bytes, respond_to);
                            tx_raw_request.send(raw_request)?;
                            let response = rx_response.await?;
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
                        _ => {
                            let response = Response::usage();
                            let _ = tx_msg.send(WsMessage::Binary(response.to_bytes()?));
                        }
                    },
                    None => break Err(anyhow!("connection closed unexpectedly")),
                }
            }
            _ = c_token.cancelled() => break Ok(()),
        }
    };
    let _ = tx_msg.send(WsMessage::Close(Some(CloseFrame {
        code: CloseCode::Normal,
        reason: Utf8Bytes::from_static("foksal closed"),
    })));
    let _ = tx_writer_cancel.send(());

    res
}

async fn run(
    port: u16,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
    tx_intra: tokio_chan::UnboundedSender<ParsedRequest<IntraRequest>>,
    c_token: CancellationToken,
) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    loop {
        tokio::select! {
            Ok((stream, _)) = listener.accept() => {
                let tx_raw_request_clone = tx_raw_request.clone();
                let tx_intra_clone = tx_intra.clone();
                let c_token_clone = c_token.clone();
                tokio::spawn(async move {
                    let res = handle_client(stream, tx_raw_request_clone, tx_intra_clone, c_token_clone).await;
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
    tx_intra: tokio_chan::UnboundedSender<ParsedRequest<IntraRequest>>,
    c_token: CancellationToken,
) -> Result<()> {
    let res = run(port, tx_raw_request, tx_intra, c_token.clone()).await;
    c_token.cancel();

    res
}
