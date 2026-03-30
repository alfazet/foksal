use base64::prelude::*;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc as tokio_chan, oneshot},
    time,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message as WsMessage};
use uuid::Uuid;

use crate::{error::FoksalError, model::*, protocol::*};

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<RawResponse>>>>;
type WsWrite = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>;
type WsRead = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct FoksalClient {
    ws_write: WsWrite,
    pending: PendingMap,
}

impl FoksalClient {
    /// Connect to a foksal instance in an async fashion.
    ///
    /// Returns the client handle and a receiver for events and asynchronous errors.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), libfoksalclient::error::FoksalError> {
    /// use libfoksalclient::{client::FoksalClient, error::FoksalError, model::SubscriptionTarget};
    ///
    /// let (mut client, mut events) = FoksalClient::connect("localhost", 2137).await?;
    ///
    /// tokio::spawn(async move {
    ///     while let Some(event) = events.recv().await {
    ///         println!("received event {:?}", event);
    ///     }
    /// });
    ///
    /// client.subscribe(SubscriptionTarget::Sink).await?;
    /// client.toggle().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(
        host: impl AsRef<str>,
        port: u16,
    ) -> Result<(Self, tokio_chan::UnboundedReceiver<AsyncMessage>), FoksalError> {
        let url = format!("ws://{}:{}", host.as_ref(), port);
        let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
        let (ws_write, ws_read) = ws_stream.split();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (tx_event, rx_event) = tokio_chan::unbounded_channel();

        let pending_clone = Arc::clone(&pending);
        tokio::spawn(async move {
            run(ws_read, pending_clone, tx_event).await;
        });

        let client = Self { ws_write, pending };

        Ok((client, rx_event))
    }

    /// Same as [connect](`Self::connect`) but with a timeout (in seconds).
    pub async fn connect_timeout(
        host: impl AsRef<str>,
        port: u16,
        timeout: Duration,
    ) -> Result<(Self, tokio_chan::UnboundedReceiver<AsyncMessage>), FoksalError> {
        time::timeout(timeout, Self::connect(host, port)).await?
    }

    /// Add songs to the playback queue.
    /// If `pos` is `None`, songs are appended to the end.
    ///
    /// Note: `pos` is zero-indexed.
    pub async fn add_to_queue(
        &mut self,
        uris: Vec<PathBuf>,
        pos: Option<usize>,
    ) -> Result<(), FoksalError> {
        self.send_no_response(Request::AddToQueue { uris, pos })
            .await
    }

    /// Remove the song at the given queue position.
    ///
    /// Note: `pos` is zero-indexed.
    pub async fn remove_from_queue(&mut self, pos: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::RemoveFromQueue { pos })
            .await
    }

    /// Move a song from one queue position to another.
    ///
    /// Note: `from` and `to` are zero-indexed.
    pub async fn queue_move(&mut self, from: usize, to: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueMove { from, to }).await
    }

    /// Append songs to the queue and immediately start playing them.
    pub async fn add_and_play(&mut self, uris: Vec<PathBuf>) -> Result<(), FoksalError> {
        self.send_no_response(Request::AddAndPlay { uris }).await
    }

    /// Clear the playback queue and stop playback.
    pub async fn queue_clear(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueClear).await
    }

    /// Start playing the song at the given queue position.
    ///
    /// Note: `pos` is zero-indexed.
    pub async fn play(&mut self, pos: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::Play { pos }).await
    }

    /// Pause playback.
    pub async fn pause(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Pause).await
    }

    /// Resume playback.
    pub async fn resume(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Resume).await
    }

    /// Toggle between playing and paused states.
    pub async fn toggle(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Toggle).await
    }

    /// Stop playback.
    pub async fn stop(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Stop).await
    }

    /// Move to the next song.
    pub async fn next(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Next).await
    }

    /// Go back to the previous song.
    pub async fn prev(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Prev).await
    }

    /// Change the volume by a relative delta.
    ///
    /// Note: the resulting volume is always clamped between 0 and 100.
    pub async fn volume_change(&mut self, delta: i8) -> Result<(), FoksalError> {
        self.send_no_response(Request::VolumeChange { delta }).await
    }

    /// Set the volume.
    ///
    /// Note: the resulting volume is always clamped between 0 and 100.
    pub async fn volume_set(&mut self, volume: u8) -> Result<(), FoksalError> {
        self.send_no_response(Request::VolumeSet { volume }).await
    }

    /// Seek within the current song. For forward/backward seeking, `seconds` must be
    /// positive/negative.
    pub async fn seek(&mut self, seconds: i64) -> Result<(), FoksalError> {
        self.send_no_response(Request::Seek { seconds }).await
    }

    /// Set the queue to sequential playback mode (see [`QueueMode`]).
    pub async fn queue_seq(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueSeq).await
    }

    /// Set the queue to random playback mode (see [`QueueMode`]).
    pub async fn queue_random(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueRandom).await
    }

    /// Set the queue to loop playback mode (see [`QueueMode`]).
    pub async fn queue_loop(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueLoop).await
    }

    /// Fetch the full current player state (see [`PlayerState`]).
    pub async fn state(&mut self) -> Result<PlayerState, FoksalError> {
        let response = self.send_with_response(Request::State).await?;
        response.into_player_state()
    }

    /// Get metadata values for specific songs.
    ///
    /// Returns a `Vec` corresponding one-to-one with the input `uris`.
    /// Entries are `None` for songs not found in the database.
    pub async fn metadata(
        &mut self,
        uris: Vec<PathBuf>,
        tags: Vec<TagKey>,
    ) -> Result<Vec<Option<SongMetadata>>, FoksalError> {
        let tags = tags.into_iter().map(|t| t.to_string()).collect();
        let response = self
            .send_with_response(Request::Metadata { uris, tags })
            .await?;
        let metadata = response.metadata.ok_or(FoksalError::UnexpectedResponse {
            request: "metadata",
        })?;
        let mut parsed_metadata = Vec::new();
        for map in metadata.into_iter() {
            if let Some(map) = map {
                let mut new_map = HashMap::new();
                for (key, json_val) in map.into_iter() {
                    let key = TagKey::try_from(key.as_str())?;
                    let val = TagValue::try_from(json_val)?;
                    new_map.insert(key, val);
                }
                parsed_metadata.push(Some(new_map));
            } else {
                parsed_metadata.push(None);
            }
        }

        Ok(parsed_metadata)
    }

    /// Fetch song URIs (with optional regex filtering and grouping).
    pub async fn select(
        &mut self,
        filters: Option<Vec<Filter>>,
        group_by: Option<Vec<TagKey>>,
    ) -> Result<Vec<SelectGroup>, FoksalError> {
        let group_by = group_by.map(|g| g.into_iter().map(|t| t.to_string()).collect());
        let response = self
            .send_with_response(Request::Select { filters, group_by })
            .await?;
        let select_groups = response.into_select_groups()?;
        let mut parsed_select_groups = Vec::new();
        for group in select_groups {
            let mut parsed_tags = HashMap::new();
            for (key, json_val) in group.tags.into_iter() {
                let key = TagKey::try_from(key.as_str())?;
                let val = TagValue::try_from(json_val)?;
                parsed_tags.insert(key, val);
            }
            parsed_select_groups.push(SelectGroup {
                uris: group.uris,
                tags: parsed_tags,
            });
        }

        Ok(parsed_select_groups)
    }

    /// Get unique values of a tag over all songs in the database (with optional grouping and
    /// sorting).
    pub async fn unique(
        &mut self,
        tag: String,
        group_by: Option<Vec<TagKey>>,
        sort: Option<SortOrder>,
    ) -> Result<Vec<UniqueGroup>, FoksalError> {
        let group_by = group_by.map(|g| g.into_iter().map(|t| t.to_string()).collect());
        let response = self
            .send_with_response(Request::Unique {
                tag,
                group_by,
                sort,
            })
            .await?;
        let unique_groups = response.into_unique_groups()?;
        let mut parsed_unique_groups = Vec::new();
        for group in unique_groups {
            let parsed_unique: Result<Vec<_>, _> =
                group.unique.into_iter().map(TagValue::try_from).collect();
            let mut parsed_tags = HashMap::new();
            for (key, json_val) in group.tags.into_iter() {
                let key = TagKey::try_from(key.as_str())?;
                let val = TagValue::try_from(json_val)?;
                parsed_tags.insert(key, val);
            }
            parsed_unique_groups.push(UniqueGroup {
                unique: parsed_unique?,
                tags: parsed_tags,
            });
        }

        Ok(parsed_unique_groups)
    }

    /// Fetch the cover art image for a song (if available).
    ///
    /// Returns `None` if the file has no embedded cover art.
    /// The returned bytes are the decoded image data.
    pub async fn cover_art(&mut self, uri: PathBuf) -> Result<Option<Vec<u8>>, FoksalError> {
        let response = self.send_with_response(Request::CoverArt { uri }).await?;
        match response.image {
            Some(encoded) => {
                let bytes = BASE64_STANDARD.decode(&encoded)?;

                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }

    /// Close the connection to foksal
    pub async fn close(&mut self) -> Result<(), FoksalError> {
        self.ws_write
            .send(WsMessage::Close(None))
            .await
            .map_err(FoksalError::WebSocket)
    }

    /// Subscribe to events emitted by the given target.
    pub async fn subscribe(&mut self, target: SubscriptionTarget) -> Result<(), FoksalError> {
        self.send_no_response(Request::Subscribe { to: target })
            .await
    }

    /// Unsubscribe from events emitted by the given target.
    pub async fn unsubscribe(&mut self, target: SubscriptionTarget) -> Result<(), FoksalError> {
        self.send_no_response(Request::Unsubscribe { to: target })
            .await
    }

    /// send a request (we care about the content of the positive response)
    async fn send_with_response(&mut self, request: Request) -> Result<RawResponse, FoksalError> {
        let token = Uuid::new_v4().to_string();
        let mut value = serde_json::to_value(&request)?;
        value["token"] = serde_json::Value::String(token.clone());
        let content = serde_json::to_vec(&value)?;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(token, tx);
        self.ws_write
            .send(WsMessage::Binary(content.into()))
            .await
            .map_err(FoksalError::WebSocket)?;
        let response = rx.await.map_err(|_| FoksalError::Disconnected)?;

        if response.ok {
            Ok(response)
        } else {
            Err(FoksalError::ServerError {
                reason: response.reason.unwrap_or_else(|| "unknown error".into()),
            })
        }
    }

    /// send a request (we only care about the possible error response)
    async fn send_no_response(&mut self, request: Request) -> Result<(), FoksalError> {
        self.send_with_response(request).await?;
        Ok(())
    }
}

async fn run(
    mut ws_read: WsRead,
    pending: PendingMap,
    tx_event: tokio_chan::UnboundedSender<AsyncMessage>,
) {
    while let Some(msg) = ws_read.next().await {
        let data = match msg {
            Ok(WsMessage::Binary(data)) => data,
            Ok(WsMessage::Close(_)) | Err(_) => break,
            _ => continue,
        };
        let parsed = match serde_json::from_slice::<FoksalMessage>(&data) {
            Ok(msg) => msg,
            Err(_) => continue,
        };
        match parsed {
            FoksalMessage::Response(response) => {
                if let Some(token) = &response.token
                    && let Some(tx) = pending.lock().unwrap().remove(token)
                {
                    let _ = tx.send(response);
                }
            }
            FoksalMessage::Event(event) => {
                let _ = tx_event.send(AsyncMessage::Event(event));
            }
            FoksalMessage::AsyncError(err) => {
                let _ = tx_event.send(AsyncMessage::Error(FoksalError::Async {
                    error: err.error,
                    reason: err.reason,
                }));
            }
            FoksalMessage::Welcome(WelcomeMessage { version }) => {
                let lib_major_version = format!("v{}", env!("CARGO_PKG_VERSION_MAJOR"));
                if !version.starts_with(&lib_major_version) {
                    let _ = tx_event.send(AsyncMessage::Error(FoksalError::VersionMismatch {
                        lib_version: env!("CARGO_PKG_VERSION").into(),
                        instance_version: version,
                    }));
                }
            }
        }
    }
}
