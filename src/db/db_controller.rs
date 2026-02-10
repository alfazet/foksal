use std::{fmt::Display, thread};

use anyhow::{Result, anyhow};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tracing::{error, instrument};

use crate::{
    db::{core::SharedDb, request::ParsedDbRequestArgs},
    net::{
        core::JsonObject,
        request::{DbRequest, ParsedRequest, RawDbRequestArgs, RawRequest, RequestKind},
        response::Response,
    },
};

fn handle_request<R: RawDbRequestArgs, P: ParsedDbRequestArgs + TryFrom<R>>(
    db: &SharedDb,
    raw_args: R,
    callback: impl Fn(&SharedDb, P) -> Response,
) -> Response
where
    <P as TryFrom<R>>::Error: Display,
{
    match raw_args.try_into() {
        Ok(parsed_args) => callback(db, parsed_args),
        Err(e) => Response::new_err(format!("argument error ({})", e)),
    }
}

fn run(db: SharedDb, mut rx_db_request: tokio_chan::UnboundedReceiver<ParsedRequest<DbRequest>>) {
    while let Some(db_request) = rx_db_request.blocking_recv() {
        let response = match db_request.request {
            DbRequest::Metadata(raw_args) => {
                handle_request(&db, raw_args, |db, parsed_args| db.metadata(parsed_args))
            }
        };
        let _ = db_request.respond_to.send(response);
    }
}

pub fn spawn_blocking(
    db: SharedDb,
    rx_db_request: tokio_chan::UnboundedReceiver<ParsedRequest<DbRequest>>,
) {
    thread::spawn(move || {
        run(db, rx_db_request);
    });
}
