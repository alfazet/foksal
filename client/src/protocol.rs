use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::FoksalError;
use crate::model::{
    Filter, PlaybackState, PlayerState, QueueMode, RawSelectGroup, RawSongMetadata, RawUniqueGroup,
    SortOrder, SubscriptionTarget,
};

/// A request sent to foksal.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "snake_case")]
pub(crate) enum Request {
    AddToQueue {
        uris: Vec<PathBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pos: Option<usize>,
    },
    RemoveFromQueue {
        pos: usize,
    },
    QueueMove {
        from: usize,
        to: usize,
    },
    AddAndPlay {
        uris: Vec<PathBuf>,
    },
    Play {
        pos: usize,
    },
    Pause,
    Resume,
    Toggle,
    Stop,
    Next,
    Prev,
    VolumeChange {
        delta: i8,
    },
    VolumeSet {
        volume: u8,
    },
    SeekBy {
        seconds: i64,
    },
    SeekTo {
        seconds: u64,
    },
    QueueSeq,
    QueueLoop,
    QueueRandom,
    QueueSingle,
    QueueClear,
    State,
    Metadata {
        uris: Vec<PathBuf>,
        tags: Vec<String>,
    },
    Select {
        #[serde(skip_serializing_if = "Option::is_none")]
        filters: Option<Vec<Filter>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        group_by: Option<Vec<String>>,
    },
    Unique {
        tag: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        group_by: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sort: Option<SortOrder>,
    },
    CoverArt {
        uri: PathBuf,
    },
    Subscribe {
        to: SubscriptionTarget,
    },
    Unsubscribe {
        to: SubscriptionTarget,
    },
}

/// A raw response from foksal.
///
/// It can be one of the following kinds:
/// 1. A success/error response to a request (identified by `ok` field)
/// 2. An async event (identified by `event` field)
/// 3. An async error (identified by `error` field)
/// 4. A welcome message (only once, identified by `version` field)
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum FoksalMessage {
    Event(Event),
    AsyncError(AsyncError),
    Response(RawResponse),
    Welcome(WelcomeMessage),
}

/// An asynchronous error that might refer to any already sent request.
#[derive(Debug, Deserialize)]
pub(crate) struct AsyncError {
    pub error: String,
    pub reason: String,
}

/// A raw response to a request.
#[derive(Debug, Deserialize)]
pub(crate) struct RawResponse {
    pub ok: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub token: Option<String>,

    // Fields from `state` response
    #[serde(default)]
    pub current_song: Option<PathBuf>,
    #[serde(default)]
    pub current_song_id: Option<usize>,
    #[serde(default)]
    pub queue_pos: Option<usize>,
    #[serde(default)]
    pub queue_mode: Option<QueueMode>,
    #[serde(default)]
    pub queue: Option<Vec<PathBuf>>,
    #[serde(default)]
    pub playback_state: Option<PlaybackState>,
    #[serde(default)]
    pub volume: Option<u8>,
    #[serde(default)]
    pub elapsed: Option<u64>,

    // Fields from `metadata` response
    #[serde(default)]
    pub metadata: Option<Vec<Option<RawSongMetadata>>>,

    // Fields from `select` and `unique` responses
    #[serde(default)]
    pub values: Option<serde_json::Value>,

    // Fields from `cover_art` response
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WelcomeMessage {
    pub version: String,
}

impl RawResponse {
    pub fn into_player_state(self) -> Result<PlayerState, FoksalError> {
        let err = || FoksalError::UnexpectedResponse { request: "state" };

        Ok(PlayerState {
            current_song: self.current_song,
            current_song_id: self.current_song_id,
            queue_pos: self.queue_pos,
            queue_mode: self.queue_mode.ok_or_else(err)?,
            queue: self.queue.ok_or_else(err)?,
            playback_state: self.playback_state.ok_or_else(err)?,
            volume: self.volume.ok_or_else(err)?,
            elapsed: self.elapsed.ok_or_else(err)?,
        })
    }

    pub fn into_select_groups(self) -> Result<Vec<RawSelectGroup>, FoksalError> {
        let v = self
            .values
            .ok_or(FoksalError::UnexpectedResponse { request: "select" })?;
        serde_json::from_value(v).map_err(FoksalError::Serialization)
    }

    pub fn into_unique_groups(self) -> Result<Vec<RawUniqueGroup>, FoksalError> {
        let v = self
            .values
            .ok_or(FoksalError::UnexpectedResponse { request: "unique" })?;
        serde_json::from_value(v).map_err(FoksalError::Serialization)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
#[serde(rename_all = "snake_case")]
/// An asynchronous event emitted by foksal to all relevant subscribers.
pub enum Event {
    /// Queue contents changed.
    QueueContent { queue: Vec<PathBuf> },
    /// Current position in the queue changed.
    QueuePos { pos: Option<usize> },
    /// The queue playback mode changed.
    QueueMode { mode: QueueMode },
    /// A new song started playing.
    CurrentSong { uri: PathBuf, id: usize },
    /// Playback state changed.
    PlaybackState { state: PlaybackState },
    /// Volume changed.
    Volume { volume: u8 },
    /// Elapsed seconds in the current song.
    Elapsed { seconds: u64 },
    /// A song was added to the database.
    Create { uri: PathBuf },
    /// A song's metadata was modified.
    Modify { uri: PathBuf },
    /// A song was removed from the database.
    Remove { uri: PathBuf },
}

#[derive(Debug)]
pub enum AsyncMessage {
    Event(Event),
    Error(FoksalError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_add_to_queue_with_pos() {
        let req = Request::AddToQueue {
            uris: vec!["a/b.flac".into()],
            pos: Some(2),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "add_to_queue");
        assert_eq!(json["pos"], 2);
        assert!(json.get("token").is_none());
    }

    #[test]
    fn serialize_add_to_queue_without_pos() {
        let req = Request::AddToQueue {
            uris: vec!["a/b.flac".into()],
            pos: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("pos").is_none());
    }

    #[test]
    fn serialize_volume() {
        let req = Request::VolumeChange { delta: -5 };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "volume_change");
        assert_eq!(json["delta"], -5);
    }

    #[test]
    fn serialize_seek_by() {
        let req = Request::SeekBy { seconds: -10 };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "seek_by");
        assert_eq!(json["seconds"], -10);
    }

    #[test]
    fn serialize_metadata_request() {
        let req = Request::Metadata {
            uris: vec!["a.flac".into()],
            tags: vec!["tracktitle".into(), "duration".into()],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "metadata");
        assert_eq!(json["tags"], serde_json::json!(["tracktitle", "duration"]));
    }

    #[test]
    fn serialize_select_request() {
        let req = Request::Select {
            filters: Some(vec![Filter {
                tag: "artist".into(),
                regex: "^Foo Bar$".into(),
            }]),
            group_by: Some(vec!["album".into()]),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "select");
        assert_eq!(json["filters"][0]["tag"], "artist");
    }

    #[test]
    fn serialize_subscribe() {
        let req = Request::Subscribe {
            to: SubscriptionTarget::Queue,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "subscribe");
        assert_eq!(json["to"], "queue");
    }

    #[test]
    fn deserialize_ok_response() {
        let json = r#"{"ok": true}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => assert!(r.ok),
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_error_response() {
        let json = r#"{"ok": false, "reason": "pos out of bounds"}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => {
                assert!(!r.ok);
                assert_eq!(r.reason.unwrap(), "pos out of bounds");
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_state_response() {
        let json = r#"{
            "ok": true,
            "current_song": "Artist/Album/01 Song.flac",
            "queue_pos": 0,
            "queue_mode": "random",
            "queue": ["Artist/Album/01 Song.flac"],
            "playback_state": "playing",
            "volume": 80,
            "elapsed": 123
        }"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => {
                let state = r.into_player_state().unwrap();
                assert_eq!(state.volume, 80);
                assert_eq!(state.queue_mode, QueueMode::Random);
                assert_eq!(state.playback_state, PlaybackState::Playing);
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_playback_state_event() {
        let json = r#"{"event": "playback_state", "state": "paused"}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Event(Event::PlaybackState { state }) => {
                assert_eq!(state, PlaybackState::Paused);
            }
            other => panic!("expected PlaybackState event, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_volume_event() {
        let json = r#"{"event": "volume", "volume": 75}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Event(Event::Volume { volume }) => {
                assert_eq!(volume, 75);
            }
            other => panic!("expected Volume event, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_elapsed_event() {
        let json = r#"{"event": "elapsed", "seconds": 42}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Event(Event::Elapsed { seconds }) => {
                assert_eq!(seconds, 42);
            }
            other => panic!("expected Elapsed event, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_async_error() {
        let json = r#"{"error": "decoder", "reason": "unsupported codec"}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::AsyncError(e) => {
                assert_eq!(e.error, "decoder");
                assert_eq!(e.reason, "unsupported codec");
            }
            other => panic!("expected AsyncError, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_create_event() {
        let json = r#"{"event": "create", "uri": "Artist/Album/New.flac"}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Event(Event::Create { uri }) => {
                assert_eq!(uri, PathBuf::from("Artist/Album/New.flac"));
            }
            other => panic!("expected Create event, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_metadata_response() {
        let json = r#"{
            "ok": true,
            "metadata": [
                {"tracktitle": "Song", "artist": "Artist", "duration": 237},
                null
            ]
        }"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => {
                let meta = r.metadata.unwrap();
                assert_eq!(meta.len(), 2);
                assert!(meta[0].is_some());
                assert!(meta[1].is_none());
                assert_eq!(meta[0].as_ref().unwrap()["tracktitle"], "Song");
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_select_response() {
        let json = r#"{
            "ok": true,
            "values": [
                {"album": "Foo", "uris": ["a.flac", "b.flac"]},
                {"album": "Bar", "uris": ["c.flac"]}
            ]
        }"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => {
                let groups = r.into_select_groups().unwrap();
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0].uris.len(), 2);
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_unique_response() {
        let json = r#"{
            "ok": true,
            "values": [
                {"genre": "Electronic", "unique": ["Foo", "Bar"]},
                {"genre": "Metal", "unique": ["Baz"]}
            ]
        }"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => {
                let groups = r.into_unique_groups().unwrap();
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[1].unique, vec!["Baz"]);
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }

    #[test]
    fn serialize_cover_art_request() {
        let req = Request::CoverArt {
            uri: "Artist/Album/01 Song.flac".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "cover_art");
        assert_eq!(json["uri"], "Artist/Album/01 Song.flac");
    }

    #[test]
    fn serialize_queue_move() {
        let req = Request::QueueMove { from: 5, to: 7 };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["kind"], "queue_move");
        assert_eq!(json["from"], 5);
        assert_eq!(json["to"], 7);
    }

    #[test]
    fn deserialize_queue_pos_event_null() {
        let json = r#"{"event": "queue_pos", "pos": null}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Event(Event::QueuePos { pos }) => {
                assert!(pos.is_none());
            }
            other => panic!("expected QueuePos event, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_response_with_token() {
        let json = r#"{"ok": true, "token": "abc123"}"#;
        let msg: FoksalMessage = serde_json::from_str(json).unwrap();
        match msg {
            FoksalMessage::Response(r) => {
                assert!(r.ok);
                assert_eq!(r.token.unwrap(), "abc123");
            }
            other => panic!("expected Response, got {other:?}"),
        }
    }
}
