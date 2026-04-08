use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt, fmt::Display, path::PathBuf};

use crate::error::FoksalError;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum QueueMode {
    /// Songs come one after another in order of the queue.
    #[default]
    Sequential,
    /// The played song will repeat indefinitely until it's manually changed.
    Loop,
    /// Songs after and including the current one are added to a pool.
    /// When the next song is requested, foksal will select one randomly from the pool.
    /// Whenever the pool empties out, it's re-initialized with all of the enqueued songs.
    Random,
    /// The playback will end after the current song finishes.
    Single,
}

/// Subscription targets for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionTarget {
    Queue,
    Sink,
    Update,
}

/// Sorting order for `unique` requests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    #[serde(alias = "asc")]
    Ascending,
    #[serde(alias = "desc")]
    Descending,
}

/// Regex filter used in `select` requests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Filter {
    pub tag: TagKey,
    pub regex: String,
}
#[derive(Debug, Serialize)]
pub(crate) struct RawFilter {
    pub tag: String,
    pub regex: String,
}

/// Full player state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct PlayerState {
    /// URI of the currently playing song or `None` if no song is playing.
    pub current_song: Option<PathBuf>,
    /// A unique ID of the currently playing song or `None` if no song is playing.
    pub current_song_id: Option<usize>,
    /// Playback queue position (0-indexed) of `None` is playback is stopped.
    pub queue_pos: Option<usize>,
    /// Playback queue mode (see [`QueueMode`]).
    pub queue_mode: QueueMode,
    /// Playback queue content.
    pub queue: Vec<PathBuf>,
    /// Playback state (see [`PlaybackState`]).
    pub playback_state: PlaybackState,
    /// Current playback volume (on a scale of 0 to 100).
    pub volume: u8,
    /// Elapsed time on the current song in seconds, 0 if no song is playing.
    pub elapsed: u64,
}

/// Available tag keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TagKey {
    Album,
    AlbumArtist,
    Artist,
    Composer,
    Date,
    DiscNumber,
    Duration,
    FileSize,
    Genre,
    Performer,
    Producer,
    SortAlbum,
    SortAlbumArtist,
    SortArtist,
    SortComposer,
    SortTrackTitle,
    TrackNumber,
    TrackTitle,
}

/// Valid types for tag values.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TagValue {
    #[default]
    Null,
    String(String),
    Number(i64),
}

/// Tag values and other file metadata of a song.
///
/// For available tag keys, see [`TagKey`].
///
/// All tags have string values, with the exception of `duration` (number of seconds) and `filesize` (number of
/// bytes).
pub type SongMetadata = HashMap<TagKey, TagValue>;
pub(crate) type RawSongMetadata = HashMap<String, Value>;

/// Group of URIs returned by the `select` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectGroup {
    /// URIs belonging to this group.
    pub uris: Vec<PathBuf>,
    /// Tag values common to this group (e.g. `{"albumartist": "ILLENIUM", "album": "Awake"}`).
    pub tags: HashMap<TagKey, TagValue>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct RawSelectGroup {
    pub uris: Vec<PathBuf>,
    #[serde(flatten)]
    pub tags: HashMap<String, Value>,
}

/// Group of unique values returned by the `unique` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGroup {
    /// Unique values of the requested tag within this group.
    pub unique: Vec<TagValue>,
    /// Values of the grouping tags.
    pub tags: HashMap<TagKey, TagValue>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct RawUniqueGroup {
    pub unique: Vec<Value>,
    #[serde(flatten)]
    pub tags: HashMap<String, Value>,
}

impl From<Filter> for RawFilter {
    fn from(f: Filter) -> Self {
        Self {
            tag: f.tag.to_string(),
            regex: f.regex,
        }
    }
}

impl Filter {
    pub fn new(tag: TagKey, regex: String) -> Self {
        Self { tag, regex }
    }

    /// Creates a filter that filters only songs having a value of `key` equal to `val`.
    /// Returns `None` if `val` is null.
    pub fn from_kv_pair(key: TagKey, val: TagValue) -> Option<Self> {
        match val {
            TagValue::Null => None,
            TagValue::String(s) => Some(Self::new(key, s)),
            TagValue::Number(n) => Some(Self::new(key, n.to_string())),
        }
    }
}

impl TryFrom<&str> for TagKey {
    type Error = FoksalError;

    fn try_from(value: &str) -> Result<Self, FoksalError> {
        match value {
            "album" => Ok(TagKey::Album),
            "albumartist" => Ok(TagKey::AlbumArtist),
            "artist" => Ok(TagKey::Artist),
            "composer" => Ok(TagKey::Composer),
            "date" => Ok(TagKey::Date),
            "discnumber" => Ok(TagKey::DiscNumber),
            "duration" => Ok(TagKey::Duration),
            "filesize" => Ok(TagKey::FileSize),
            "genre" => Ok(TagKey::Genre),
            "performer" => Ok(TagKey::Performer),
            "producer" => Ok(TagKey::Producer),
            "sortalbum" => Ok(TagKey::SortAlbum),
            "sortalbumartist" => Ok(TagKey::SortAlbumArtist),
            "sortartist" => Ok(TagKey::SortArtist),
            "sortcomposer" => Ok(TagKey::SortComposer),
            "sorttracktitle" => Ok(TagKey::SortTrackTitle),
            "tracknumber" => Ok(TagKey::TrackNumber),
            "tracktitle" => Ok(TagKey::TrackTitle),
            other => Err(FoksalError::InvalidTagKey(other.to_string())),
        }
    }
}

impl Display for TagKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TagKey::Album => "album",
            TagKey::AlbumArtist => "albumartist",
            TagKey::Artist => "artist",
            TagKey::Composer => "composer",
            TagKey::Date => "date",
            TagKey::DiscNumber => "discnumber",
            TagKey::Duration => "duration",
            TagKey::FileSize => "filesize",
            TagKey::Genre => "genre",
            TagKey::Performer => "performer",
            TagKey::Producer => "producer",
            TagKey::SortAlbum => "sortalbum",
            TagKey::SortAlbumArtist => "sortalbumartist",
            TagKey::SortArtist => "sortartist",
            TagKey::SortComposer => "sortcomposer",
            TagKey::SortTrackTitle => "sorttracktitle",
            TagKey::TrackNumber => "tracknumber",
            TagKey::TrackTitle => "tracktitle",
        };

        write!(f, "{}", s)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_from_kv_pair() {
        let key = TagKey::Artist;
        let val = TagValue::String("FooBar".into());
        let filter = Filter::new(key, "FooBar".into());

        assert_eq!(filter, Filter::from_kv_pair(key, val).unwrap())
    }
}
