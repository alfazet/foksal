use anyhow::{Result, anyhow};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    thread,
};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tracing::{error, instrument};

use crate::{
    net::{
        request::{ParsedRequest, PlayerRequest, RawPlayerRequestArgs},
        response::Response,
    },
    player::{core::Player, request::ParsedPlayerRequestArgs},
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

fn run(
    mut player: Player,
    mut rx_player_request: tokio_chan::UnboundedReceiver<ParsedRequest<PlayerRequest>>,
) {
    while let Some(player_request) = rx_player_request.blocking_recv() {
        let response = match player_request.request {
            PlayerRequest::AddToQueue(raw_args) => {
                handle_request_mut(&mut player, raw_args, |player, parsed_args| {
                    player.add_to_queue(parsed_args)
                })
            }
            PlayerRequest::State => player.state(),
            _ => todo!(),
        };
        let _ = player_request.respond_to.send(response);
    }
}

pub fn spawn_blocking(
    player: Player,
    rx_player_request: tokio_chan::UnboundedReceiver<ParsedRequest<PlayerRequest>>,
) {
    thread::spawn(move || {
        run(player, rx_player_request);
    });
}
