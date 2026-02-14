use anyhow::{Result, anyhow};
use std::{fmt::Display, thread};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tracing::{error, instrument};

use crate::{
    db::{
        core::SharedDb,
        request::{DbRequest, DbRequestKind, ParsedDbRequestArgs},
    },
    net::{
        core::JsonObject,
        request::{RawDbRequest, RawDbRequestArgs, SubscribeArgs, UnsubscribeArgs},
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

// TODO: maybe make it so that only the "expensive" requests are blocking
fn run(db: SharedDb, mut rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>) {
    while let Some(DbRequest { kind, respond_to }) = rx_db_request.blocking_recv() {
        let response = match kind {
            DbRequestKind::Raw(raw_request) => match raw_request {
                RawDbRequest::Metadata(raw_args) => {
                    handle_request(&db, raw_args, |db, parsed_args| db.metadata(parsed_args))
                }
                RawDbRequest::Select(raw_args) => {
                    handle_request(&db, raw_args, |db, parsed_args| db.select(parsed_args))
                }
                _ => unreachable!(),
            },
            DbRequestKind::Subscribe(SubscribeArgs {
                target,
                addr,
                send_to,
            }) => {
                db.add_subscriber(target, addr, send_to);
                Response::new_ok()
            }
            DbRequestKind::Unsubscribe(UnsubscribeArgs { target, addr }) => {
                db.remove_subscriber(target, addr);
                Response::new_ok()
            }
        };
        let _ = respond_to.send(response);
    }
}

pub fn spawn_blocking(db: SharedDb, rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>) {
    thread::spawn(move || {
        run(db, rx_db_request);
    });
}
