use mpris_server::{
    LocalPlayerInterface, LocalRootInterface, LocalServer, LoopStatus, Metadata, PlaybackRate,
    PlaybackStatus, Time, TrackId, Volume,
    builder::MetadataBuilder,
    zbus::{Result, fdo},
};
use serde_json::Value;
use std::{path::PathBuf, thread};
use tokio::{
    runtime::Runtime,
    sync::{mpsc as tokio_chan, oneshot},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

use libfoksalcommon::{
    net::{
        request::{
            LocalRequestKind, MprisRequest, RawAddAndPlayArgs, RawDbRequest, RawMetadataArgs,
            RawPlayerRequest, RawSeekByArgs, RawSeekToArgs, RawVolumeSetArgs,
        },
        response::Response,
    },
    utils::{self},
};

const MPRIS_TAGS: [&str; 9] = [
    "album",
    "albumartist",
    "artist",
    "composer",
    "discnumber",
    "duration",
    "genre",
    "tracktitle",
    "tracknumber",
];

pub struct FoksalMpris {
    tx_request: tokio_chan::UnboundedSender<MprisRequest>,
    c_token: CancellationToken,
}

impl FoksalMpris {
    fn new(
        tx_request: tokio_chan::UnboundedSender<MprisRequest>,
        c_token: CancellationToken,
    ) -> Self {
        Self {
            tx_request,
            c_token,
        }
    }

    async fn player_state(&self) -> fdo::Result<Response> {
        let (respond_to, rx) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::State),
            respond_to,
        });
        let resp = rx
            .await
            .map_err(|_| fdo::Error::Failed("player_state fetch".into()))?;

        Ok(resp)
    }
}

impl LocalRootInterface for FoksalMpris {
    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.c_token.cancel();
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _: bool) -> Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("foksal".into())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("foksal".into())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".into()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/aac".into(),
            "audio/flac".into(),
            "audio/mpeg".into(),
            "audio/wav".into(),
        ])
    }
}

impl LocalPlayerInterface for FoksalMpris {
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
            "sequential" | "random" => LoopStatus::None,
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
        let tags: Vec<_> = MPRIS_TAGS.iter().map(|t| t.to_string()).collect();
        let args = RawMetadataArgs { uris, tags };
        let (respond_to, rx) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::DbRequest(RawDbRequest::Metadata(args)),
            respond_to,
        });
        let resp = rx
            .await
            .map_err(|_| fdo::Error::Failed("metadata fetch".into()))?;
        let Some(metadata) = resp.inner()["metadata"].as_array().and_then(|m| m.first()) else {
            return Err(fdo::Error::Failed("metadata deserialization".into()));
        };
        let track_id: TrackId = utils::uri_to_track_id(uri, id as usize)
            .try_into()
            .map_err(|_| fdo::Error::Failed("invalid track id".into()))?;

        convert_metadata(track_id, metadata)
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

pub fn spawn(tx_request: tokio_chan::UnboundedSender<MprisRequest>, c_token: CancellationToken) {
    let foksal_mpris = FoksalMpris::new(tx_request, c_token);
    thread::spawn(move || {
        let rt = Runtime::new().expect("falied to create tokio runtime for MPRIS");
        rt.block_on(async move {
            let server = match LocalServer::new("org.mpris.MediaPlayer2.foksal", foksal_mpris).await
            {
                Ok(server) => server,
                Err(e) => {
                    error!("failed to start MPRIS server: {}", e);
                    return;
                }
            };
            server.run().await;

            // TODO: emit events
            //  - on playback status change
            //  - on loop status change
            //  - on shuffle status change
            //  - on song change (new metadata)
            //  - on volume change
        });
    });
}

/// convert a metadata map from json to mpris format
fn convert_metadata(track_id: TrackId, json: &Value) -> fdo::Result<Metadata> {
    let metadata = json
        .as_object()
        .ok_or(fdo::Error::Failed("metadata deserialization".into()))?;
    let album = metadata["album"].as_str();
    let albumartist = metadata["albumartist"].as_str();
    let artist = metadata["artist"].as_str();
    let composer = metadata["composer"].as_str();
    let discnumber = metadata["discnumber"].as_i64();
    let duration = metadata["duration"].as_i64();
    let genre = metadata["genre"].as_str();
    let tracktitle = metadata["tracktitle"].as_str();
    let tracknumber = metadata["tracknumber"].as_i64();

    let mut builder = MetadataBuilder::default().trackid(track_id);
    if let Some(album) = album {
        builder = builder.album(album);
    }
    if let Some(albumartist) = albumartist {
        builder = builder.album_artist([albumartist]);
    }
    if let Some(artist) = artist {
        builder = builder.artist([artist]);
    }
    if let Some(composer) = composer {
        builder = builder.composer([composer]);
    }
    if let Some(discnumber) = discnumber {
        builder = builder.disc_number(discnumber as i32);
    }
    if let Some(duration) = duration {
        builder = builder.length(Time::from_secs(duration));
    }
    if let Some(genre) = genre {
        builder = builder.genre([genre]);
    }
    if let Some(tracktitle) = tracktitle {
        builder = builder.title(tracktitle);
    }
    if let Some(tracknumber) = tracknumber {
        builder = builder.track_number(tracknumber as i32)
    }

    Ok(builder.build())
}
