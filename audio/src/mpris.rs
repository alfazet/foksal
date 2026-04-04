use mpris_server::{
    LocalPlayerInterface, LocalRootInterface, LocalServer, LoopStatus, Metadata, PlaybackRate,
    PlaybackStatus, Time, TrackId, Volume,
    zbus::{Result, fdo},
};
use std::thread;
use tokio::{
    runtime::Runtime,
    sync::{mpsc as tokio_chan, oneshot},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

use libfoksalcommon::net::request::{LocalRequestKind, MprisRequest, RawPlayerRequest};

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
        todo!()
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        todo!()
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        todo!()
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let (respond_to, rx) = oneshot::channel();
        let _ = self.tx_request.send(MprisRequest {
            kind: LocalRequestKind::PlayerRequest(RawPlayerRequest::State),
            respond_to,
        });

        let resp = rx
            .await
            .map_err(|_| fdo::Error::Failed("playback_status".into()))?;
        let Some(playback_status) = resp.inner()["playback_state"].as_str() else {
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
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> Result<()> {
        todo!()
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, rate: PlaybackRate) -> Result<()> {
        todo!()
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, shuffle: bool) -> Result<()> {
        todo!()
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(Metadata::new())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(1.0)
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        todo!()
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_secs(0))
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
        });
    });
}
