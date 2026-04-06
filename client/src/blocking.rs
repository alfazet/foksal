use base64::prelude::*;
use std::{collections::HashMap, net::TcpStream, path::PathBuf};
use tokio_tungstenite::tungstenite::{
    self, Message as WsMessage, WebSocket, stream::MaybeTlsStream,
};

use crate::{error::FoksalError, model::*, protocol::*};

type WsStream = WebSocket<MaybeTlsStream<TcpStream>>;

pub struct BlockingFoksalClient {
    stream: WsStream,
}

impl BlockingFoksalClient {
    /// Connect to a foksal instance in a blocking (non-async) fashion.
    pub fn connect(host: impl AsRef<str>, port: u16) -> Result<Self, FoksalError> {
        let url = format!("ws://{}:{}", host.as_ref(), port);
        let (mut stream, _) = tungstenite::connect(&url)?;
        let first_msg = match stream.read()? {
            WsMessage::Binary(bytes) => match serde_json::from_slice::<FoksalMessage>(&bytes) {
                Ok(msg) => msg,
                _ => return Err(FoksalError::InvalidWelcome),
            },
            _ => return Err(FoksalError::InvalidWelcome),
        };
        match first_msg {
            FoksalMessage::Welcome(WelcomeMessage { version }) => {
                let lib_major_version = format!("v{}", env!("CARGO_PKG_VERSION_MAJOR"));
                if !version.starts_with(&lib_major_version) {
                    return Err(FoksalError::VersionMismatch {
                        lib_version: env!("CARGO_PKG_VERSION").into(),
                        instance_version: version,
                    });
                }
            }
            _ => return Err(FoksalError::InvalidWelcome),
        }

        Ok(Self { stream })
    }

    /// Add songs to the playback queue.
    /// If `pos` is `None`, songs are appended to the end.
    ///
    /// Note: `pos` is zero-indexed.
    pub fn add_to_queue(
        &mut self,
        uris: Vec<PathBuf>,
        pos: Option<usize>,
    ) -> Result<(), FoksalError> {
        self.send_no_response(Request::AddToQueue { uris, pos })
    }

    /// Remove the song at the given queue position.
    ///
    /// Note: `pos` is zero-indexed.
    pub fn remove_from_queue(&mut self, pos: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::RemoveFromQueue { pos })
    }

    /// Move a song from one queue position to another.
    ///
    /// Note: `from` and `to` are zero-indexed.
    pub fn queue_move(&mut self, from: usize, to: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueMove { from, to })
    }

    /// Append songs to the queue and immediately start playing them.
    pub fn add_and_play(&mut self, uris: Vec<PathBuf>) -> Result<(), FoksalError> {
        self.send_no_response(Request::AddAndPlay { uris })
    }

    /// Clear the playback queue and stop playback.
    pub fn queue_clear(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueClear)
    }

    /// Start playing the song at the given queue position.
    ///
    /// Note: `pos` is zero-indexed.
    pub fn play(&mut self, pos: usize) -> Result<(), FoksalError> {
        self.send_no_response(Request::Play { pos })
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Pause)
    }

    /// Resume playback.
    pub fn resume(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Resume)
    }

    /// Toggle between playing and paused states.
    pub fn toggle(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Toggle)
    }

    /// Stop playback.
    pub fn stop(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Stop)
    }

    /// Move to the next song.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Next)
    }

    /// Go back to the previous song.
    pub fn prev(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::Prev)
    }

    /// Change the volume by a relative delta.
    ///
    /// Note: the resulting volume is always clamped between 0 and 100.
    pub fn volume_change(&mut self, delta: i8) -> Result<(), FoksalError> {
        self.send_no_response(Request::VolumeChange { delta })
    }

    /// Set the volume.
    ///
    /// Note: the resulting volume is always clamped between 0 and 100.
    pub fn volume_set(&mut self, volume: u8) -> Result<(), FoksalError> {
        self.send_no_response(Request::VolumeSet { volume })
    }

    /// Seek by an offset within the current song. For forward/backward seeking, `seconds` must be
    /// positive/negative.
    pub fn seek_by(&mut self, seconds: i64) -> Result<(), FoksalError> {
        self.send_no_response(Request::SeekBy { seconds })
    }

    /// Seek by absolute position within the current song.
    pub fn seek_to(&mut self, seconds: u64) -> Result<(), FoksalError> {
        self.send_no_response(Request::SeekTo { seconds })
    }

    /// Set the queue to sequential playback mode (see [`QueueMode`]).
    pub fn queue_seq(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueSeq)
    }

    /// Set the queue to loop playback mode (see [`QueueMode`]).
    pub fn queue_loop(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueLoop)
    }

    /// Set the queue to random playback mode (see [`QueueMode`]).
    pub fn queue_random(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueRandom)
    }

    /// Set the queue to single playback mode (see [`QueueMode`]).
    pub fn queue_single(&mut self) -> Result<(), FoksalError> {
        self.send_no_response(Request::QueueSingle)
    }

    /// Fetch the full current player state (see [`PlayerState`]).
    pub fn state(&mut self) -> Result<PlayerState, FoksalError> {
        let response = self.send_with_response(Request::State)?;
        response.into_player_state()
    }

    /// Get metadata values for specific songs.
    ///
    /// Returns a `Vec` corresponding one-to-one with the input `uris`.
    /// Entries are `None` for songs not found in the database.
    pub fn metadata(
        &mut self,
        uris: Vec<PathBuf>,
        tags: Vec<TagKey>,
    ) -> Result<Vec<Option<SongMetadata>>, FoksalError> {
        let tags = tags.into_iter().map(|t| t.to_string()).collect();
        let response = self.send_with_response(Request::Metadata { uris, tags })?;
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
    pub fn select(
        &mut self,
        filters: Option<Vec<Filter>>,
        group_by: Option<Vec<String>>,
    ) -> Result<Vec<SelectGroup>, FoksalError> {
        let group_by = group_by.map(|g| g.into_iter().map(|t| t.to_string()).collect());
        let response = self.send_with_response(Request::Select { filters, group_by })?;
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

    pub fn unique(
        &mut self,
        tag: String,
        group_by: Option<Vec<String>>,
        sort: Option<SortOrder>,
    ) -> Result<Vec<UniqueGroup>, FoksalError> {
        let group_by = group_by.map(|g| g.into_iter().map(|t| t.to_string()).collect());
        let response = self.send_with_response(Request::Unique {
            tag,
            group_by,
            sort,
        })?;
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
    pub fn cover_art(&mut self, uri: PathBuf) -> Result<Option<Vec<u8>>, FoksalError> {
        let response = self.send_with_response(Request::CoverArt { uri })?;
        match response.image {
            Some(encoded) => {
                let bytes = BASE64_STANDARD.decode(&encoded)?;

                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }

    /// Close the connection to foksal
    pub fn close(&mut self) -> Result<(), FoksalError> {
        self.stream
            .send(WsMessage::Close(None))
            .map_err(FoksalError::WebSocket)
    }

    /// send a request (we care about the content of the positive response)
    fn send_with_response(&mut self, request: Request) -> Result<RawResponse, FoksalError> {
        let content = serde_json::to_vec(&request)?;
        self.stream
            .send(WsMessage::Binary(content.into()))
            .map_err(FoksalError::WebSocket)?;

        // ignore all async errors, return whenever we get the actual response
        loop {
            let response = match self.stream.read().map_err(|_| FoksalError::Disconnected) {
                Ok(WsMessage::Binary(data)) => Some(data),
                Ok(WsMessage::Text(text)) => Some(text.as_bytes().to_vec().into()),
                _ => None,
            };
            let parsed = match response {
                Some(data) => serde_json::from_slice::<FoksalMessage>(&data)?,
                _ => return Err(FoksalError::Disconnected),
            };
            match parsed {
                FoksalMessage::Response(response) => break Ok(response),
                _ => continue,
            }
        }
    }

    /// send a request (we only care about the possible error response)
    fn send_no_response(&mut self, request: Request) -> Result<(), FoksalError> {
        self.send_with_response(request)?;
        Ok(())
    }
}
