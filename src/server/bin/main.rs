use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc as tokio_chan,
};
use tokio_tungstenite::{tungstenite::Message as WsMessage, tungstenite::Utf8Bytes};

async fn handle_client(stream: TcpStream) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let (msg_tx, mut msg_rx) = tokio_chan::unbounded_channel();

    let sender_task = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            let _ = ws_tx.send(msg).await;
        }
    });

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(WsMessage::Text(payload)) => {
                eprintln!("client pinged the server with {}", payload);
                let _ = msg_tx.send(WsMessage::Text(Utf8Bytes::from_static("response")));
            }
            Ok(WsMessage::Close(_)) | Err(_) => {
                eprintln!("the client dc'ed");
                break;
            }
            _ => todo!(),
        }
    }

    sender_task.abort();
    eprintln!("client handling finished");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:2137").await?;
    while let Ok((stream, _)) = listener.accept().await {
        eprintln!("accepted a new client");
        tokio::spawn(handle_client(stream));
    }

    Ok(())
}
