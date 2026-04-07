use anyhow::Result as AnyhowResult;
use mpris_server::{
    LoopStatus, Metadata, PlaybackStatus, Property, Server, Time, TrackId,
    builder::MetadataBuilder,
    zbus::{Error as ZbusError, Result, fdo},
};
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{core::PlayerEvent, queue::QueueMode, sink::PlaybackState};
use libfoksalcommon::{
    net::{
        request::{
            LocalRequestKind, MprisRequest, RawDbRequest, RawMetadataArgs, RawPlayerRequest,
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
    pub tx_request: tokio_chan::UnboundedSender<MprisRequest>,
    pub c_token: CancellationToken,
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

    pub async fn player_state(&self) -> fdo::Result<Response> {
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

pub async fn spawn(
    tx_request: tokio_chan::UnboundedSender<MprisRequest>,
    mut rx_event: tokio_chan::UnboundedReceiver<PlayerEvent>,
    c_token: CancellationToken,
) -> AnyhowResult<()> {
    let tx_request_clone = tx_request.clone();
    let foksal_mpris = FoksalMpris::new(tx_request_clone, c_token);
    let server = Server::new(
        format!("org.mpris.MediaPlayer2.foksal{}", std::process::id()).as_str(),
        foksal_mpris,
    )
    .await?;

    tokio::spawn(async move {
        while let Some(event) = rx_event.recv().await {
            match event {
                PlayerEvent::QueueMode { mode } => match mode {
                    QueueMode::Sequential => {
                        let _ = server
                            .properties_changed([
                                Property::Shuffle(false),
                                Property::LoopStatus(LoopStatus::None),
                            ])
                            .await;
                    }
                    QueueMode::Loop => {
                        let _ = server
                            .properties_changed([
                                Property::Shuffle(false),
                                Property::LoopStatus(LoopStatus::Track),
                            ])
                            .await;
                    }
                    QueueMode::Random | QueueMode::Single => {
                        let _ = server
                            .properties_changed([
                                Property::Shuffle(true),
                                Property::LoopStatus(LoopStatus::None),
                            ])
                            .await;
                    }
                },
                PlayerEvent::PlaybackState { state } => match state {
                    PlaybackState::Stopped => {
                        let _ = server
                            .properties_changed([Property::PlaybackStatus(PlaybackStatus::Stopped)])
                            .await;
                    }
                    PlaybackState::Paused => {
                        let _ = server
                            .properties_changed([Property::PlaybackStatus(PlaybackStatus::Paused)])
                            .await;
                    }
                    _ => {
                        let _ = server
                            .properties_changed([Property::PlaybackStatus(PlaybackStatus::Playing)])
                            .await;
                    }
                },
                PlayerEvent::Volume { volume } => {
                    let volume = (volume as f64) / 100.0;
                    let _ = server.properties_changed([Property::Volume(volume)]).await;
                }
                PlayerEvent::CurrentSong { uri, id } => {
                    if let Some(metadata) = fetch_metadata(&tx_request, vec![uri], vec![id])
                        .await
                        .ok()
                        .and_then(|m| m.first().cloned())
                    {
                        let _ = server
                            .properties_changed([Property::Metadata(metadata)])
                            .await;
                    }
                }
                _ => unreachable!(),
            }
        }
    });

    Ok(())
}

/// convert a metadata map from json to the format expected by mpris
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

pub async fn fetch_metadata(
    tx_request: &tokio_chan::UnboundedSender<MprisRequest>,
    uris: Vec<PathBuf>,
    ids: Vec<usize>,
) -> fdo::Result<Vec<Metadata>> {
    let tags: Vec<_> = MPRIS_TAGS.iter().map(|t| t.to_string()).collect();
    let args = RawMetadataArgs {
        uris: uris.clone(),
        tags,
    };
    let (respond_to, rx) = oneshot::channel();
    let _ = tx_request.send(MprisRequest {
        kind: LocalRequestKind::DbRequest(RawDbRequest::Metadata(args)),
        respond_to,
    });
    let resp = rx
        .await
        .map_err(|_| fdo::Error::Failed("metadata fetch".into()))?;
    let Some(metadata) = resp.inner()["metadata"].as_array() else {
        return Err(fdo::Error::Failed("metadata deserialization".into()));
    };
    let res: Result<Vec<_>> = metadata
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let track_id: TrackId = utils::uri_to_track_id(uris[i].as_path(), ids[i])
                .try_into()
                .map_err(|_| fdo::Error::Failed("invalid track id".into()))?;

            convert_metadata(track_id, item).map_err(|e| ZbusError::FDO(Box::new(e)))
        })
        .collect();

    Ok(res?)
}
