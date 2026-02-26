use crossbeam_channel as cbeam_chan;
use std::fmt::Display;
use tokio::sync::mpsc as tokio_chan;

use crate::{
    core::Player,
    request::{ParsedPlayerRequestArgs, PlayerRequest, PlayerRequestKind},
    sink::{SinkRequest, SinkResponse},
};
use libfoksalcommon::net::{
    request::{RawPlayerRequest, RawPlayerRequestArgs, SubscribeArgs, UnsubscribeArgs},
    response::Response,
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
    tx_sink_request: cbeam_chan::Sender<SinkRequest>,
    mut rx_player_request: tokio_chan::UnboundedReceiver<PlayerRequest>,
    mut rx_sink_response: tokio_chan::UnboundedReceiver<SinkResponse>,
) {
    let mut player = Player::new(tx_sink_request);
    loop {
        tokio::select! {
            Some(PlayerRequest { kind, respond_to }) = rx_player_request.recv() => {
                let response = match kind {
                    PlayerRequestKind::Raw(raw_request) => match raw_request {
                        RawPlayerRequest::AddToQueue(raw_args) => {
                            handle_request_mut(&mut player, raw_args, |player, args| {
                                player.req_add_to_queue(args)
                            })
                        }
                        RawPlayerRequest::Play(raw_args) => {
                            handle_request_mut(&mut player, raw_args, |player, args| player.req_play(args))
                        }
                        RawPlayerRequest::Pause => player.req_pause().await,
                        RawPlayerRequest::Resume => player.req_resume().await,
                        RawPlayerRequest::Toggle => player.req_toggle().await,
                        RawPlayerRequest::Stop => player.req_stop().await,
                        RawPlayerRequest::Next => player.req_next().await,
                        RawPlayerRequest::Prev => player.req_prev().await,
                        RawPlayerRequest::QueueSeq => player.req_queue_seq(),
                        RawPlayerRequest::QueueRandom => player.req_queue_random(),
                        _ => unreachable!(), // subscription requests are handled below
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
            Some(response) = rx_sink_response.recv() => {
                match response {
                    SinkResponse::SongOver => {
                        player.next().await;
                    }
                }
            }
            else => {
                break;
            }
        }
    }
}

pub fn spawn(
    tx_sink_request: cbeam_chan::Sender<SinkRequest>,
    rx_player_request: tokio_chan::UnboundedReceiver<PlayerRequest>,
    rx_sink_response: tokio_chan::UnboundedReceiver<SinkResponse>,
) {
    tokio::spawn(async move {
        run(tx_sink_request, rx_player_request, rx_sink_response).await;
    });
}
