use anyhow::{Result, anyhow};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    thread,
};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tracing::{error, instrument};
use uuid::Uuid;

use crate::{
    net::{
        request::{
            ParsedRequest, RawAddToQueueArgs, RawPlayerRequest, RawPlayerRequestArgs,
            SubscribeArgs, UnsubscribeArgs,
        },
        response::Response,
    },
    player::{
        core::Player,
        request::{ParsedPlayerRequestArgs, PlayerRequest, PlayerRequestKind},
    },
};

fn handle_request<R: RawPlayerRequestArgs, P: ParsedPlayerRequestArgs + TryFrom<R>>(
    player: &Player,
    raw_args: R,
    callback: impl Fn(&Player, P) -> Response,
) -> Response
where
    <P as TryFrom<R>>::Error: Display,
{
    match raw_args.try_into() {
        Ok(parsed_args) => callback(player, parsed_args),
        Err(e) => Response::new_err(format!("argument error ({})", e)),
    }
}

fn handle_request_mut<R: RawPlayerRequestArgs, P: ParsedPlayerRequestArgs + TryFrom<R>>(
    player: &mut Player,
    raw_args: R,
    callback: impl Fn(&mut Player, P) -> Response,
) -> Response
where
    <P as TryFrom<R>>::Error: Display,
{
    match raw_args.try_into() {
        Ok(parsed_args) => callback(player, parsed_args),
        Err(e) => Response::new_err(format!("argument error ({})", e)),
    }
}

async fn run(
    mut player: Player,
    mut rx_player_request: tokio_chan::UnboundedReceiver<PlayerRequest>,
) {
    while let Some(PlayerRequest { kind, respond_to }) = rx_player_request.recv().await {
        let response = match kind {
            PlayerRequestKind::Raw(raw_request) => match raw_request {
                RawPlayerRequest::AddToQueue(raw_args) => {
                    handle_request_mut(&mut player, raw_args, |player, args| {
                        player.add_to_queue(args)
                    })
                }
                _ => unreachable!(),
            },
            PlayerRequestKind::Subscribe(SubscribeArgs {
                target,
                addr,
                send_to,
            }) => {
                player.add_subscriber(target, addr, send_to);
                Response::new_ok()
            }
            PlayerRequestKind::Unsubscribe(UnsubscribeArgs { target, addr }) => {
                player.remove_subscriber(target, addr);
                Response::new_ok()
            }
        };
        let _ = respond_to.send(response);
    }
}

pub fn spawn(player: Player, rx_player_request: tokio_chan::UnboundedReceiver<PlayerRequest>) {
    tokio::spawn(async move {
        run(player, rx_player_request).await;
    });
}
