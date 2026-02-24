use anyhow::Result;
use globset::GlobSet;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc as tokio_chan;

use crate::{
    core::{Db, SharedDb},
    decoder::Decoder,
    request::{DbRequest, DbRequestKind, ParsedDbRequestArgs},
};
use libfoksalcommon::net::{
    request::{FileRequest, RawDbRequest, RawDbRequestArgs, SubscribeArgs, UnsubscribeArgs},
    response::Response,
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

async fn run(db: SharedDb, mut rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>) {
    while let Some(DbRequest { kind, respond_to }) = rx_db_request.recv().await {
        let response = match kind {
            DbRequestKind::Raw(raw_request) => match raw_request {
                RawDbRequest::Metadata(raw_args) => {
                    handle_request(&db, raw_args, |db, parsed_args| {
                        db.req_metadata(parsed_args)
                    })
                }
                RawDbRequest::Select(raw_args) => {
                    handle_request(&db, raw_args, |db, parsed_args| db.req_select(parsed_args))
                }
                _ => unreachable!(), // subscription requests are handled below
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
    music_root: impl AsRef<Path> + Into<PathBuf>,
    ignore_globset: GlobSet,
    allowed_exts: Vec<String>,
    rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>,
    rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>,
) -> Result<()> {
    let db = Db::new(music_root.as_ref(), &ignore_globset, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(music_root.as_ref(), ignore_globset, allowed_exts)?;
    tokio::spawn(async move {
        run(db, rx_db_request).await;
    });

    let decoder = Decoder::new(music_root);
    tokio::spawn(async move {
        decoder.run(rx_file_request).await;
    });

    Ok(())
}
