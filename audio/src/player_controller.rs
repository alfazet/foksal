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

#[cfg(feature = "mpris")]
use crate::core::PlayerEvent;

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
    #[cfg(feature = "mpris")] tx_mpris_event: tokio_chan::UnboundedSender<PlayerEvent>,
    mut rx_player_request: tokio_chan::UnboundedReceiver<PlayerRequest>,
    mut rx_sink_response: tokio_chan::UnboundedReceiver<SinkResponse>,
) {
    let mut player = Player::new(
        tx_sink_request,
        #[cfg(feature = "mpris")]
        tx_mpris_event,
    );
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
                        RawPlayerRequest::RemoveFromQueue(raw_args) => {
                            handle_request_mut(&mut player, raw_args, |player, args| {
                                player.req_remove_from_queue(args)
                            })
                        }
                        RawPlayerRequest::QueueMove(raw_args) => {
                            handle_request_mut(&mut player, raw_args, |player, args| {
                                player.req_queue_move(args)
                            })
                        }
                        RawPlayerRequest::AddAndPlay(raw_args) => {
                            handle_request_mut(&mut player, raw_args, |player, args| {
                                player.req_add_and_play(args)
                            })
                        }
                        RawPlayerRequest::Play(raw_args) => {
                            handle_request_mut(&mut player, raw_args, |player, args| player.req_play(args))
                        }
                        RawPlayerRequest::VolumeChange(raw_args) => {
                            handle_request(&player, raw_args, |player, args| player.req_volume_change(args))
                        }
                        RawPlayerRequest::VolumeSet(raw_args) => {
                            handle_request(&player, raw_args, |player, args| player.req_volume_set(args))
                        }
                        RawPlayerRequest::SeekBy(raw_args) => {
                            handle_request(&player, raw_args, |player, args| player.req_seek_by(args))
                        }
                        RawPlayerRequest::SeekTo(raw_args) => {
                            handle_request(&player, raw_args, |player, args| player.req_seek_to(args))
                        }
                        RawPlayerRequest::State => player.req_state().await,
                        RawPlayerRequest::Pause => player.req_pause(),
                        RawPlayerRequest::Resume => player.req_resume(),
                        RawPlayerRequest::Toggle => player.req_toggle(),
                        RawPlayerRequest::Stop => player.req_stop(),
                        RawPlayerRequest::Next => player.req_next(),
                        RawPlayerRequest::Prev => player.req_prev(),
                        RawPlayerRequest::QueueSeq => player.req_queue_seq(),
                        RawPlayerRequest::QueueLoop => player.req_queue_loop(),
                        RawPlayerRequest::QueueRandom => player.req_queue_random(),
                        RawPlayerRequest::QueueSingle => player.req_queue_single(),
                        RawPlayerRequest::QueueClear => player.req_queue_clear(),
                        RawPlayerRequest::Subscribe(_) | RawPlayerRequest::Unsubscribe(_) => unreachable!(),
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
                        player.next();
                    }
                    SinkResponse::StateChanged(state) => {
                        player.notify_playback_state(state);
                    }
                    SinkResponse::VolumeChanged(volume) => {
                        player.notify_volume(volume);
                    }
                    SinkResponse::Elapsed(seconds) => {
                        player.notify_elapsed(seconds);
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
    #[cfg(feature = "mpris")] tx_mpris_event: tokio_chan::UnboundedSender<PlayerEvent>,
    rx_player_request: tokio_chan::UnboundedReceiver<PlayerRequest>,
    rx_sink_response: tokio_chan::UnboundedReceiver<SinkResponse>,
) {
    tokio::spawn(async move {
        run(
            tx_sink_request,
            #[cfg(feature = "mpris")]
            tx_mpris_event,
            rx_player_request,
            rx_sink_response,
        )
        .await;
    });
}
