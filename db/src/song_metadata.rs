use anyhow::{Result, anyhow};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use symphonia::core::meta::MetadataRevision;

use crate::{
    filter::ParsedFilter,
    fs_utils,
    tag::{ExtendedTagKey, TagKey},
};

#[derive(Clone, Debug, Default)]
pub struct SongMetadata {
    items: HashMap<TagKey, String>,
}

impl From<&MetadataRevision> for SongMetadata {
    fn from(revision: &MetadataRevision) -> Self {
        let mut items = HashMap::new();
        for tag in revision.tags() {
            if let Some(tag_key) = tag.std_key.and_then(|key| TagKey::try_from(key).ok()) {
                items
                    .entry(tag_key)
                    .or_insert_with(|| tag.value.to_string());
            }
        }

        Self { items }
    }
}

impl SongMetadata {
    pub fn try_new(uri: impl AsRef<Path>, root: impl Into<PathBuf>) -> Result<Self> {
        let abs_path = fs_utils::to_absolute(&uri, root);
        let mut probe_res = fs_utils::get_probe_result(&abs_path)?;
        let from_container = probe_res
            .format
            .metadata()
            .current()
            .map(SongMetadata::from)
            .unwrap_or_default();
        let from_probe = probe_res
            .metadata
            .get()
            .map(|m| m.current().map(SongMetadata::from).unwrap_or_default())
            .unwrap_or_default();
        // merge both sources because different formats
        // store data in different places
        let mut data = from_container.merge(from_probe);

        let demuxer = probe_res.format;
        let track = demuxer.default_track().ok_or(anyhow!(
            "no audio track found in `{}`",
            uri.as_ref().to_string_lossy()
        ))?;
        if let Some(tb) = &track.codec_params.time_base
            && let Some(n) = &track.codec_params.n_frames
        {
            data.items.insert(
                TagKey::Extended(ExtendedTagKey::Duration),
                tb.calc_time(*n).seconds.to_string(),
            );
        }
        if let Ok(size) = fs::metadata(abs_path).map(|meta| meta.len()) {
            data.items
                .insert(TagKey::Extended(ExtendedTagKey::FileSize), size.to_string());
        }

        Ok(data)
    }

    pub fn get(&self, tag_key: &TagKey) -> Option<&str> {
        self.items.get(tag_key).map(|x| x.as_str())
    }

    pub fn matches(&self, filters: &[ParsedFilter]) -> bool {
        filters.iter().all(|filter| {
            self.get(&filter.tag)
                .map(|value| filter.regex.is_match(value))
                .is_some_and(|x| x)
        })
    }

    fn merge(self, other: Self) -> Self {
        Self {
            items: self.items.into_iter().chain(other.items).collect(),
        }
    }
}
