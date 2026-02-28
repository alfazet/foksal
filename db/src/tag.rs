use anyhow::{Result, bail};
use lazy_static::lazy_static;
use std::{cmp::Ordering, collections::HashMap, fmt::Display};
use symphonia::core::meta::StandardTagKey;

pub static N_SUPPORTED_TAGS: usize = 5;

const TAG_NAMES: [&str; N_SUPPORTED_TAGS] = [
    "album",
    "albumartist",
    "artist",
    "tracknumber",
    "tracktitle",
];

const TAG_KEYS: [StandardTagKey; N_SUPPORTED_TAGS] = [
    StandardTagKey::Album,
    StandardTagKey::AlbumArtist,
    StandardTagKey::Artist,
    StandardTagKey::TrackNumber,
    StandardTagKey::TrackTitle,
];

lazy_static! {
    static ref TAG_MAP: HashMap<&'static str, StandardTagKey> = {
        TAG_NAMES
            .iter()
            .cloned()
            .zip(TAG_KEYS.iter().cloned())
            .collect()
    };
    static ref INVERSE_TAG_MAP: HashMap<StandardTagKey, &'static str> = {
        TAG_KEYS
            .iter()
            .cloned()
            .zip(TAG_NAMES.iter().cloned())
            .collect()
    };
    static ref TAG_FALLBACK_RULES: HashMap<StandardTagKey, StandardTagKey> =
        HashMap::from([(StandardTagKey::AlbumArtist, StandardTagKey::Artist),]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TagKey(StandardTagKey);

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

        Ok(Self(key))
    }
}

impl TryFrom<StandardTagKey> for TagKey {
    type Error = anyhow::Error;

    fn try_from(st_key: StandardTagKey) -> Result<Self> {
        if TAG_KEYS.contains(&st_key) {
            Ok(Self(st_key))
        } else {
            bail!("unsupported tag key `{:?}`", st_key);
        }
    }
}

impl Display for TagKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", INVERSE_TAG_MAP.get(&self.0).unwrap())
    }
}

impl TagKey {
    pub fn fallback(self) -> Option<Self> {
        TAG_FALLBACK_RULES.get(&self.0).copied().map(Self)
    }

    pub fn cmp(&self, a: Option<&str>, b: Option<&str>) -> Ordering {
        match (a, b) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => match self.0 {
                StandardTagKey::TrackNumber => {
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
        let key = TagKey(StandardTagKey::TrackNumber);

        assert_eq!(key.cmp(a, b), Ordering::Less);
        assert_eq!(key.cmp(b, c), Ordering::Less);
        assert_eq!(key.cmp(a, c), Ordering::Less);
        assert_eq!(key.cmp(a, a), Ordering::Equal);
    }
}
