use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{Bytes, Message as WsMessage, Utf8Bytes};

#[tokio::main]
async fn main() -> Result<()> {
    let (ws_stream, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:2137").await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    let reader_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(WsMessage::Text(response)) => {
                    eprintln!("ping came back with response {}", response);
                }
                _ => todo!(),
            }
        }
    });

    ws_tx
        .send(WsMessage::Text(Utf8Bytes::from_static("hi")))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    ws_tx.send(WsMessage::Close(None)).await?;

    Ok(())
}
