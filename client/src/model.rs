use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub current_song: Option<String>,
    pub queue_pos: Option<usize>,
    pub queue_mode: QueueMode,
    pub queue: Vec<String>,
    pub sink_state: PlaybackState,
    pub volume: usize,
    /// In seconds.
    pub elapsed: u64,
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
pub type SongMetadata = HashMap<String, serde_json::Value>;

/// Group of URIs returned by the `select` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectGroup {
    /// URIs belonging to this group.
    pub uris: Vec<String>,
    /// Tag values common to this group (e.g. `{"albumartist": "ILLENIUM", "album": "Awake"}`).
    #[serde(flatten)]
    pub tags: HashMap<String, serde_json::Value>,
}

/// Group of unique values returned by the `unique` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniqueGroup {
    /// Unique values of the requested tag within this group.
    pub unique: Vec<String>,
    /// Values of the grouping tags.
    #[serde(flatten)]
    pub tags: HashMap<String, serde_json::Value>,
}
