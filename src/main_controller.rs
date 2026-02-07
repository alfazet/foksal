use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::sync::oneshot;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc as tokio_chan,
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_util::sync::CancellationToken;
use tracing::{Level, error, event, info};

use crate::net::request::RawRequest;

async fn handle_client(
    stream: TcpStream,
    tx_raw_request: tokio_chan::UnboundedSender<RawRequest>,
) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (msg_tx, mut msg_rx) = tokio_chan::unbounded_channel();
    let (writer_cancel_tx, mut writer_cancel_rx) = oneshot::channel();

    let writer_task = tokio::spawn(async move {
        loop {
            if let Some(msg) = msg_rx.recv().await {
                let _ = ws_write.send(msg).await;
            }
            if writer_cancel_rx.try_recv().is_ok() {
                break;
            }
        }
    });

    while let Some(msg) = ws_read.next().await {
        match msg {
            Ok(WsMessage::Binary(bytes)) => {
                println!("received {} bytes", bytes.len());
            }
            Ok(WsMessage::Close(_)) => {
                info!("client closed the connection");
                break;
            }
            Err(e) => {
                error!("{}", e);
                break;
            }
            _ => (),
        }
    }
    let _ = writer_cancel_tx.send(());

    Ok(())
}

// TODO: pass full config
async fn run(port: u16, tx_raw_request: tokio_chan::UnboundedSender<RawRequest>) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    while let Ok((stream, _)) = listener.accept().await {
        let tx_request_data = tx_raw_request.clone();
        tokio::spawn(async move {
            let res = handle_client(stream, tx_request_data.clone()).await;
            if let Err(e) = res {
                error!("fatal error ({})", e);
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
            error!("fatal error ({})", e);
        }
        // finish everything when the controller ends
        c_token.cancel();
    })
}
