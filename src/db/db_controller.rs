use anyhow::{Result, anyhow};
use tokio::{
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument};

use crate::{
    db::core::SharedDb,
    net::{core::JsonObject, request::RawRequest, response::Response},
};

async fn run(
    db: SharedDb,
    mut rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
) -> Result<()> {
    while let Some(raw_request) = rx_raw_request.recv().await {
        let request: Result<JsonObject> =
            serde_json::from_slice(raw_request.data()).map_err(|e| anyhow!(e));
        let respond_to = raw_request.respond_to;
        match request {
            Ok(request) => {
                println!("db controller received:\n{:#?}", request);
                let _ = respond_to.send(Response::new_ok());
            }
            Err(_) => {
                println!("db controller received non-JSON garbage");
                let _ = respond_to.send(Response::new_err("you sent me garbage"));
            }
        }
    }

    Ok(())
}

pub fn spawn(
    db: SharedDb,
    rx_raw_request: tokio_chan::UnboundedReceiver<RawRequest>,
    c_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let res = tokio::select! {
            res = run(db, rx_raw_request) => res,
            _ = c_token.cancelled() => Ok(()),
        };
        if let Err(e) = res {
            error!("db controller error ({})", e);
        }
        c_token.cancel();
    })
}
