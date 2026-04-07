use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Time, TrackId, Volume,
    builder::MetadataBuilder,
    zbus::{Result, fdo},
};
use std::path::PathBuf;
use tokio::sync::oneshot;

use crate::mpris::core::{FoksalMpris, fetch_metadata};
use libfoksalcommon::{
    net::request::{
        LocalRequestKind, MprisRequest, RawAddAndPlayArgs, RawPlayerRequest, RawSeekByArgs,
        RawSeekToArgs, RawVolumeSetArgs,
    },
    utils::{self},
};

impl PlayerInterface for FoksalMpris {
    async fn next(&self) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::Next),
            respond_to,
        });

        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::Prev),
            respond_to,
        });

        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::Pause),
            respond_to,
        });

        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::Toggle),
            respond_to,
        });

        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::Stop),
            respond_to,
        });

        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::Resume),
            respond_to,
        });

        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let args = RawSeekByArgs {
            seconds: offset.as_secs() as isize,
        };
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::SeekBy(args)),
            respond_to,
        });

        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        let player_state = self.player_state().await?;
        let (Some(uri), Some(id)) = (
            player_state.inner()["current_song"].as_str(),
            player_state.inner()["current_song_id"].as_u64(),
        ) else {
            return Ok(());
        };
        if utils::uri_to_track_id(uri, id as usize) != track_id.as_str() {
            return Ok(());
        }

        let (respond_to, _) = oneshot::channel();
        let args = RawSeekToArgs {
            seconds: position.as_secs() as usize,
        };
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::SeekTo(args)),
            respond_to,
        });

        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        let (respond_to, _) = oneshot::channel();
        let args = RawAddAndPlayArgs {
            uris: vec![PathBuf::from(uri)],
        };
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::AddAndPlay(args)),
            respond_to,
        });

        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let player_state = self.player_state().await?;
        let Some(playback_status) = player_state.inner()["playback_state"].as_str() else {
            return Err(fdo::Error::Failed("playback_status deserialization".into()));
        };
        let res = match playback_status {
            "playing" => PlaybackStatus::Playing,
            "paused" => PlaybackStatus::Paused,
            "stopped" => PlaybackStatus::Stopped,
            _ => unreachable!(),
        };

        Ok(res)
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        let player_state = self.player_state().await?;
        let Some(queue_mode) = player_state.inner()["queue_mode"].as_str() else {
            return Err(fdo::Error::Failed("loop_status deserialization".into()));
        };
        let res = match queue_mode {
            "sequential" | "random" | "single" => LoopStatus::None,
            "loop" => LoopStatus::Track,
            _ => unreachable!(),
        };

        Ok(res)
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(match loop_status {
                LoopStatus::Track => RawPlayerRequest::QueueLoop,
                _ => RawPlayerRequest::QueueSeq,
            }),
            respond_to,
        });

        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _: PlaybackRate) -> Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        let player_state = self.player_state().await?;
        let Some(queue_mode) = player_state.inner()["queue_mode"].as_str() else {
            return Err(fdo::Error::Failed("loop_status deserialization".into()));
        };
        let res = queue_mode == "random";

        Ok(res)
    }

    async fn set_shuffle(&self, shuffle: bool) -> Result<()> {
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(if shuffle {
                RawPlayerRequest::QueueRandom
            } else {
                RawPlayerRequest::QueueSeq
            }),
            respond_to,
        });

        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let player_state = self.player_state().await?;
        let (Some(uri), Some(id)) = (
            player_state.inner()["current_song"].as_str(),
            player_state.inner()["current_song_id"].as_u64(),
        ) else {
            let no_track = MetadataBuilder::default().trackid(TrackId::NO_TRACK);
            return Ok(no_track.build());
        };

        let uris = vec![PathBuf::from(uri)];
        let ids = vec![id as usize];
        match fetch_metadata(&self.tx_request, uris, ids).await {
            Ok(mut metadata) => Ok(metadata.swap_remove(0)),
            Err(e) => Err(e),
        }
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        let player_state = self.player_state().await?;
        let Some(volume) = player_state.inner()["volume"].as_i64() else {
            return Err(fdo::Error::Failed("volume deserialization".into()));
        };
        let volume = volume as f64 / 100.0;

        Ok(volume)
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        let volume = ((volume * 100.0) as u8).clamp(0, 100);
        let args = RawVolumeSetArgs { volume };
        let (respond_to, _) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::VolumeSet(args)),
            respond_to,
        });

        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        let player_state = self.player_state().await?;
        let Some(elapsed) = player_state.inner()["elapsed"].as_i64() else {
            return Err(fdo::Error::Failed("position deserialization".into()));
        };
        let elapsed = Time::from_secs(elapsed);

        Ok(elapsed)
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
