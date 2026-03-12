use base64::prelude::*;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    net::TcpStream,
    sync::{Mutex, mpsc as tokio_chan, oneshot},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};
use uuid::Uuid;

use crate::{error::FoksalError, model::*, protocol::*};

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<RawResponse>>>>;
type WsWrite = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsRead = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct FoksalClient {
    sink: Arc<Mutex<WsWrite>>,
    pending: PendingMap,
}

impl FoksalClient {
    /// Connect to a foksal instance.
    ///
    /// Returns the client handle and a receiver for events and asynchronous errors.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), libfoksalclient::error::FoksalError> {
    /// use libfoksalclient::{client::FoksalClient, error::FoksalError, model::SubscriptionTarget};
    ///
    /// let (client, mut events) = FoksalClient::connect("localhost", 2137).await?;
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
        let (event_tx, event_rx) = tokio_chan::unbounded_channel();

        let pending_clone = Arc::clone(&pending);
        tokio::spawn(async move {
            run_reader(ws_read, pending_clone, event_tx).await;
        });

        let client = Self {
            sink: Arc::new(Mutex::new(ws_write)),
            pending,
        };

        Ok((client, event_rx))
    }

    /// Add songs to the playback queue.
    /// If `pos` is `None`, songs are appended to the end.
    ///
    /// Note: `pos` is zero-indexed.
    pub async fn add_to_queue(
        &self,
        uris: Vec<String>,
        pos: Option<usize>,
    ) -> Result<(), FoksalError> {
        self.send_no_response(Request::AddToQueue { uris, pos })
            .await
    }

    /// Remove the song at the given queue position.
    ///
    /// Note: `pos` is zero-indexed.
    pub async fn remove_from_queue(&self, pos: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::RemoveFromQueue { pos })
            .await
    }

    /// Move a song from one queue position to another.
    ///
    /// Note: `to` and `from` are zero-indexed.
    pub async fn queue_move(&self, from: usize, to: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueMove { from, to }).await
    }

    /// Append songs to the queue and immediately start playing them.
    pub async fn add_and_play(&self, uris: Vec<String>) -> Result<(), FoksalError> {
        self.send_no_response(Request::AddAndPlay { uris }).await
    }

    /// Clear the playback queue and stop playback.
    pub async fn queue_clear(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueClear {}).await
    }

    /// Start playing the song at the given queue position.
    ///
    /// Note: `pos` is zero-indexed.
    pub async fn play(&self, pos: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::Play { pos }).await
    }

    /// Pause playback.
    pub async fn pause(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Pause {}).await
    }

    /// Resume playback.
    pub async fn resume(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Resume {}).await
    }

    /// Toggle between playing and paused states.
    pub async fn toggle(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Toggle {}).await
    }

    /// Stop playback.
    pub async fn stop(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Stop {}).await
    }

    /// Move to the next song.
    pub async fn next(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Next {}).await
    }

    /// Go back to the previous song.
    pub async fn prev(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Prev {}).await
    }

    /// Change the volume by a relative delta.
    ///
    /// Note: the resulting volume is always clamped between 0 and 100.
    pub async fn volume(&self, delta: i8) -> Result<(), FoksalError> {
        self.send_no_response(Request::Volume { delta }).await
    }

    /// Seek within the current song. For forward/backward seeking, `seconds` must be
    /// positive/negative.
    pub async fn seek(&self, seconds: i64) -> Result<(), FoksalError> {
        self.send_no_response(Request::Seek { seconds }).await
    }

    /// Set the queue to sequential playback mode (see [`QueueMode`]).
    pub async fn queue_seq(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueSeq {}).await
    }

    /// Set the queue to random playback mode (see [`QueueMode`]).
    pub async fn queue_random(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueRandom {}).await
    }

    /// Set the queue to loop playback mode (see [`QueueMode`]).
    pub async fn queue_loop(&self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueLoop {}).await
    }

    /// Fetch the full current player state (see [`PlayerState`]).
    pub async fn state(&self) -> Result<PlayerState, FoksalError> {
        let response = self.send_with_response(Request::State {}).await?;
        response
            .into_player_state()
            .ok_or_else(|| FoksalError::ProtocolError("invalid `state` response".into()))
    }

    /// Get metadata values for specific songs.
    ///
    /// Returns a `Vec` corresponding one-to-one with the input `uris`.
    /// Entries are `None` for songs not found in the database.
    pub async fn metadata(
        &self,
        uris: Vec<String>,
        tags: Vec<String>,
    ) -> Result<Vec<Option<SongMetadata>>, FoksalError> {
        let response = self
            .send_with_response(Request::Metadata { uris, tags })
            .await?;

        response
            .metadata
            .ok_or_else(|| FoksalError::ProtocolError("invalid `metadata` response".into()))
    }

    /// Fetch song URIs (with optional regex filtering and grouping).
    pub async fn select(
        &self,
        filters: Option<Vec<Filter>>,
        group_by: Option<Vec<String>>,
    ) -> Result<Vec<SelectGroup>, FoksalError> {
        let response = self
            .send_with_response(Request::Select { filters, group_by })
            .await?;

        response
            .into_select_groups()
            .ok_or_else(|| FoksalError::ProtocolError("invalid `select` response".into()))
    }

    /// Get unique values of a tag over all songs in the database (with optional grouping and
    /// sorting).
    pub async fn unique(
        &self,
        tag: String,
        group_by: Option<Vec<String>>,
        sort: Option<SortOrder>,
    ) -> Result<Vec<UniqueGroup>, FoksalError> {
        let response = self
            .send_with_response(Request::Unique {
                tag,
                group_by,
                sort,
            })
            .await?;

        response
            .into_unique_groups()
            .ok_or_else(|| FoksalError::ProtocolError("invalid `unique` response".into()))
    }

    /// Fetch the cover art image for a song (if available).
    ///
    /// Returns `None` if the file has no embedded cover art.
    /// The returned bytes are the decoded image data.
    pub async fn cover_art(&self, uri: String) -> Result<Option<Vec<u8>>, FoksalError> {
        let response = self.send_with_response(Request::CoverArt { uri }).await?;
        match response.image {
            Some(encoded) => {
                let bytes = BASE64_STANDARD.decode(&encoded).map_err(|e| {
                    FoksalError::ProtocolError(format!("base64 decoding error ({})", e))
                })?;

                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }

    /// Subscribe to events emitted by the given target.
    pub async fn subscribe(&self, target: SubscriptionTarget) -> Result<(), FoksalError> {
        self.send_no_response(Request::Subscribe { to: target })
            .await
    }

    /// Unsubscribe from events emitted by the given target.
    pub async fn unsubscribe(&self, target: SubscriptionTarget) -> Result<(), FoksalError> {
        self.send_no_response(Request::Unsubscribe { to: target })
            .await
    }

    /// send a request (we care about the content of the positive response)
    async fn send_with_response(&self, request: Request) -> Result<RawResponse, FoksalError> {
        let token = Uuid::new_v4().to_string();
        let mut value = serde_json::to_value(&request)?;
        value["token"] = serde_json::Value::String(token.clone());
        let content = serde_json::to_vec(&value)?;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(token, tx);
        self.sink
            .lock()
            .await
            .send(Message::Binary(content.into()))
            .await
            .map_err(FoksalError::ConnectionFailed)?;
        let response = rx.await.map_err(|_| FoksalError::Disconnected)?;

        if response.ok {
            Ok(response)
        } else {
            Err(FoksalError::ServerError(
                response.reason.unwrap_or_else(|| "unknown error".into()),
            ))
        }
    }

    /// send a request (we only care about the possible error response)
    async fn send_no_response(&self, request: Request) -> Result<(), FoksalError> {
        self.send_with_response(request).await?;
        Ok(())
    }
}

async fn run_reader(
    mut ws_read: WsRead,
    pending: PendingMap,
    event_tx: tokio_chan::UnboundedSender<AsyncMessage>,
) {
    while let Some(msg) = ws_read.next().await {
        let data = match msg {
            Ok(Message::Binary(data)) => data,
            Ok(Message::Text(text)) => text.as_bytes().to_vec().into(),
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(_) => break,
        };
        let parsed = match serde_json::from_slice::<FoksalMessage>(&data) {
            Ok(m) => m,
            Err(_) => continue,
        };
        match parsed {
            FoksalMessage::Response(response) => {
                if let Some(token) = &response.token
                    && let Some(tx) = pending.lock().await.remove(token)
                {
                    let _ = tx.send(response);
                }
            }
            FoksalMessage::Event(event) => {
                let _ = event_tx.send(AsyncMessage::Event(event));
            }
            FoksalMessage::AsyncError(err) => {
                let _ = event_tx.send(AsyncMessage::Error(AsyncError {
                    error: err.error,
                    reason: err.reason,
                }));
            }
            FoksalMessage::Welcome(WelcomeMessage { version }) => {
                let lib_major_version = format!("v{}", env!("CARGO_PKG_VERSION_MAJOR"));
                if !version.starts_with(&lib_major_version) {
                    let _ = event_tx.send(AsyncMessage::Error(AsyncError {
                        error: "major version incompatiblity".into(),
                        reason: format!(
                            "libfoksal {} is incompatible with foksal {}",
                            env!("CARGO_PKG_VERSION"),
                            version
                        ),
                    }));
                }
            }
        }
    }
}
