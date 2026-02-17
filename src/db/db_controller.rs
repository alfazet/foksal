use anyhow::{Result, anyhow};
use std::{fmt::Display, thread};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;
use tracing::{error, instrument};

use crate::{
    config::DbConfig,
    db::{
        core::{Db, SharedDb},
        request::{DbRequest, DbRequestKind, ParsedDbRequestArgs},
    },
    net::{
        core::JsonObject,
        request::{RawDbRequest, RawDbRequestArgs, RawFileRequest, SubscribeArgs, UnsubscribeArgs},
        response::Response,
    },
    player::request::FileRequest,
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

async fn run(
    db: SharedDb,
    mut rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>,
    mut rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>,
) {
    loop {
        tokio::select! {
            db_request = rx_db_request.recv() => {
                match db_request {
                    Some(DbRequest { kind, respond_to}) => {
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
                    None => break,
                }
            }
            file_request = rx_file_request.recv() => {
                match file_request {
                    Some(FileRequest { raw, respond_to }) => {
                        match raw {
                            RawFileRequest::PrepareFile(uri) => {
                                println!("preparing {:?}", uri);
                            }
                            RawFileRequest::GetChunk { uri, .. } => {
                                if let Some(respond_to) = respond_to {
                                    let _ = respond_to.send(Bytes::from_static(b"lorem ipsum"));
                                }
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }

    while let Some(DbRequest { kind, respond_to }) = rx_db_request.recv().await {
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

pub fn spawn(
    config: DbConfig,
    rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>,
    rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>,
) -> Result<()> {
    let DbConfig {
        music_root,
        ignore_glob_set,
        allowed_exts,
    } = config;
    let db = Db::new(&music_root, &ignore_glob_set, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_root, ignore_glob_set, allowed_exts)?;

    tokio::spawn(async move {
        run(db, rx_db_request, rx_file_request).await;
    });

    Ok(())
}
