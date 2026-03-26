use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf};

use crate::error::FoksalError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueMode {
    /// Songs come one after another in order of the queue.
    Sequential,
    /// Songs after and including the current one are added to a pool.
    /// When the next song is requested, foksal will select one randomly from the pool.
    /// Whenever the pool empties out, it's re-initialized with all of the enqueued songs.
    Random,
    /// The played song will repeat indefinitely until it's manually changed.
    Loop,
}

/// Subscription targets for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionTarget {
    Queue,
    Sink,
    Update,
}

/// Sorting order for `unique` requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[serde(alias = "asc")]
    Ascending,
    #[serde(alias = "desc")]
    Descending,
}

/// Regex filter used in `select` requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    pub tag: String,
    pub regex: String,
}

/// Full player state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub current_song: Option<PathBuf>,
    pub queue_pos: Option<usize>,
    pub queue_mode: QueueMode,
    pub queue: Vec<PathBuf>,
    pub playback_state: PlaybackState,
    pub volume: u8,
    /// In seconds.
    pub elapsed: u64,
}

/// Valid types for tag values.
#[derive(Debug, Clone)]
pub enum TagValue {
    Null,
    String(String),
    Number(i64),
}

/// Tag values and other file metadata of a song.
///
/// Available keys:
/// - `album`
/// - `albumartist`
/// - `artist`
/// - `composer`
/// - `date`
/// - `discnumber`
/// - `duration`
/// - `filesize`
/// - `genre`
/// - `performer`
/// - `producer`
/// - `sortalbum`
/// - `sortalbumartist`
/// - `sortartist`
/// - `sortcomposer`
/// - `sorttracktitle`
/// - `tracknumber`
/// - `tracktitle`
///
/// All of the above have string values, with the exception of `duration` (number of seconds) and `filesize` (number of
/// bytes).
pub type SongMetadata = HashMap<String, TagValue>;
pub(crate) type RawSongMetadata = HashMap<String, Value>;

/// Group of URIs returned by the `select` request.
pub struct SelectGroup {
    /// URIs belonging to this group.
    pub uris: Vec<PathBuf>,
    /// Tag values common to this group (e.g. `{"albumartist": "ILLENIUM", "album": "Awake"}`).
    pub tags: HashMap<String, TagValue>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct RawSelectGroup {
    pub uris: Vec<PathBuf>,
    #[serde(flatten)]
    pub tags: HashMap<String, Value>,
}

/// Group of unique values returned by the `unique` request.
pub struct UniqueGroup {
    /// Unique values of the requested tag within this group.
    pub unique: Vec<TagValue>,
    /// Values of the grouping tags.
    pub tags: HashMap<String, TagValue>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct RawUniqueGroup {
    pub unique: Vec<Value>,
    #[serde(flatten)]
    pub tags: HashMap<String, Value>,
}

impl TryFrom<Value> for TagValue {
    type Error = FoksalError;

    fn try_from(value: Value) -> Result<Self, FoksalError> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::String(s) => Ok(Self::String(s)),
            Value::Number(n) => Ok(Self::Number(
                n.as_i64().expect("numbers should fit in an i64"),
            )),
            Value::Bool(_) => Err(FoksalError::InvalidTagValue("bool".into())),
            Value::Array(_) => Err(FoksalError::InvalidTagValue("array".into())),
            Value::Object(_) => Err(FoksalError::InvalidTagValue("object".into())),
        }
    }
}

impl TagValue {
    /// Returns unit if `self` is a `null`, or `None` otherwise.
    pub fn as_null(&self) -> Option<()> {
        if let Self::Null = self {
            Some(())
        } else {
            None
        }
    }

    /// Returns a str reference if `self` is a `String`, or `None` otherwise.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s.as_str())
        } else {
            None
        }
    }

    /// Returns an `i64` if `self` is a `Number`, or `None` otherwise.
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Number(n) = self {
            Some(*n)
        } else {
            None
        }
    }
}
