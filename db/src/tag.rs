use anyhow::{Result, bail};
use lazy_static::lazy_static;
use std::{cmp::Ordering, collections::HashMap, fmt::Display};
use symphonia::core::meta::StandardTagKey;

pub static N_STANDARD_TAGS: usize = 16;
pub static N_EXTENDED_TAGS: usize = 2;

const ST_TAG_NAMES: [&str; N_STANDARD_TAGS] = [
    "album",
    "albumartist",
    "artist",
    "composer",
    "date",
    "discnumber",
    "genre",
    "performer",
    "producer",
    "sortalbum",
    "sortalbumartist",
    "sortartist",
    "sortcomposer",
    "sorttracktitle",
    "tracknumber",
    "tracktitle",
];

const EXT_TAG_NAMES: [&str; N_EXTENDED_TAGS] = ["duration", "filesize"];

const ST_TAG_KEYS: [StandardTagKey; N_STANDARD_TAGS] = [
    StandardTagKey::Album,
    StandardTagKey::AlbumArtist,
    StandardTagKey::Artist,
    StandardTagKey::Composer,
    StandardTagKey::Date,
    StandardTagKey::DiscNumber,
    StandardTagKey::Genre,
    StandardTagKey::Performer,
    StandardTagKey::Producer,
    StandardTagKey::SortAlbum,
    StandardTagKey::SortAlbumArtist,
    StandardTagKey::SortArtist,
    StandardTagKey::SortComposer,
    StandardTagKey::SortTrackTitle,
    StandardTagKey::TrackNumber,
    StandardTagKey::TrackTitle,
];

const EXT_TAG_KEYS: [ExtendedTagKey; N_EXTENDED_TAGS] =
    [ExtendedTagKey::Duration, ExtendedTagKey::FileSize];

lazy_static! {
    static ref TAG_MAP: HashMap<&'static str, TagKey> = {
        ST_TAG_NAMES
            .iter()
            .copied()
            .zip(ST_TAG_KEYS.iter().copied().map(TagKey::Standard))
            .chain(
                EXT_TAG_NAMES
                    .iter()
                    .copied()
                    .zip(EXT_TAG_KEYS.iter().copied().map(TagKey::Extended)),
            )
            .collect()
    };
    static ref INVERSE_TAG_MAP: HashMap<TagKey, &'static str> = {
        ST_TAG_KEYS
            .iter()
            .copied()
            .map(TagKey::Standard)
            .zip(ST_TAG_NAMES.iter().copied())
            .chain(
                EXT_TAG_KEYS
                    .iter()
                    .copied()
                    .map(TagKey::Extended)
                    .zip(EXT_TAG_NAMES.iter().copied()),
            )
            .collect()
    };
    static ref TAG_FALLBACK_RULES: HashMap<StandardTagKey, StandardTagKey> = HashMap::from([
        (StandardTagKey::SortAlbum, StandardTagKey::Album),
        (StandardTagKey::SortAlbumArtist, StandardTagKey::AlbumArtist),
        (StandardTagKey::SortArtist, StandardTagKey::Artist),
        (StandardTagKey::SortComposer, StandardTagKey::Composer),
        (StandardTagKey::SortTrackTitle, StandardTagKey::TrackTitle),
        (StandardTagKey::AlbumArtist, StandardTagKey::Artist),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ExtendedTagKey {
    Duration,
    FileSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TagKey {
    Standard(StandardTagKey),
    Extended(ExtendedTagKey),
}

#[derive(Clone, Copy)]
pub enum SortingOrder {
    Ascending,
    Descending,
}

impl TryFrom<&str> for TagKey {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        let Some(key) = TAG_MAP.get(&s).cloned() else {
            bail!("invalid tag key `{}`", s);
        };

        Ok(key)
    }
}

impl TryFrom<StandardTagKey> for TagKey {
    type Error = anyhow::Error;

    fn try_from(st_key: StandardTagKey) -> Result<Self> {
        if ST_TAG_KEYS.contains(&st_key) {
            Ok(Self::Standard(st_key))
        } else {
            bail!("unsupported tag key `{:?}`", st_key);
        }
    }
}

impl TryFrom<ExtendedTagKey> for TagKey {
    type Error = anyhow::Error;

    fn try_from(ext_key: ExtendedTagKey) -> Result<Self> {
        if EXT_TAG_KEYS.contains(&ext_key) {
            Ok(Self::Extended(ext_key))
        } else {
            bail!("unsupported tag key `{:?}`", ext_key);
        }
    }
}

impl Display for TagKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", INVERSE_TAG_MAP.get(self).unwrap())
    }
}

impl TagKey {
    pub fn fallback(self) -> Option<Self> {
        match self {
            Self::Standard(st) => TAG_FALLBACK_RULES.get(&st).copied().map(Self::Standard),
            Self::Extended(_) => None,
        }
    }

    pub fn cmp(&self, a: Option<&str>, b: Option<&str>) -> Ordering {
        match (a, b) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => match self {
                Self::Standard(StandardTagKey::DiscNumber)
                | Self::Standard(StandardTagKey::TrackNumber) => {
                    let a = parse_out_of(a);
                    let b = parse_out_of(b);
                    match (a, b) {
                        (Some(a), Some(b)) => a.cmp(&b),
                        _ => Ordering::Equal,
                    }
                }
                _ => a.cmp(b),
            },
        }
    }
}

impl TryFrom<&str> for SortingOrder {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        match s {
            "asc" | "ascending" => Ok(Self::Ascending),
            "desc" | "descending" => Ok(Self::Descending),
            other => bail!("invalid sorting order `{}`", other),
        }
    }
}

/// parses XX if `s` is of the form "XX/YY", or the entirity of `s` otherwise
fn parse_out_of(s: &str) -> Option<i32> {
    match s.split_once('/') {
        Some((s, _)) => s.parse::<i32>().ok(),
        None => s.parse::<i32>().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmp() {
        let a = Some("03/12");
        let b = Some("4");
        let c = Some("10/12");
        let key = TagKey::Standard(StandardTagKey::TrackNumber);

        assert_eq!(key.cmp(a, b), Ordering::Less);
        assert_eq!(key.cmp(b, c), Ordering::Less);
        assert_eq!(key.cmp(a, c), Ordering::Less);
        assert_eq!(key.cmp(a, a), Ordering::Equal);
    }
}
