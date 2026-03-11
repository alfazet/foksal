use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
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
use foksalcommon::net::{
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
                RawDbRequest::Unique(raw_args) => {
                    handle_request(&db, raw_args, |db, parsed_args| db.req_unique(parsed_args))
                }
                RawDbRequest::CoverArt(raw_args) => {
                    handle_request(&db, raw_args, |db, parsed_args| {
                        db.req_cover_art(parsed_args)
                    })
                }
                RawDbRequest::Subscribe(_) | RawDbRequest::Unsubscribe(_) => unreachable!(),
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
    ignore_globset: Vec<Glob>,
    allowed_exts: Vec<String>,
    rx_db_request: tokio_chan::UnboundedReceiver<DbRequest>,
    rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>,
) -> Result<()> {
    let mut globset_builder = GlobSetBuilder::new();
    for glob in ignore_globset {
        globset_builder.add(glob);
    }
    let ignore_globset = globset_builder.build()?;
    let music_root = dunce::canonicalize(music_root.as_ref())?;
    let db = Db::new(&music_root, &ignore_globset, &allowed_exts)?;
    let db = SharedDb::new(db);
    db.start_fs_watcher(&music_root, ignore_globset, allowed_exts)?;
    tokio::spawn(async move {
        run(db, rx_db_request).await;
    });

    let decoder = Decoder::new(music_root);
    tokio::spawn(async move {
        decoder.run(rx_file_request).await;
    });

    Ok(())
}
