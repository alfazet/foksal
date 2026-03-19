use anyhow::{Result, anyhow};
use base64::prelude::*;
use serde_json::Value;
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
    items: HashMap<TagKey, Value>,
    uri: PathBuf, // absolute path
}

impl SongMetadata {
    pub fn try_new(uri: impl AsRef<Path>, root: impl Into<PathBuf>) -> Result<Self> {
        let abs_path = fs_utils::to_absolute(&uri, root);
        let mut probe_res = fs_utils::get_probe_result(&abs_path)?;
        // TODO: refactor this process
        let from_container = probe_res
            .format
            .metadata()
            .current()
            .map(|r| SongMetadata::from_revision(r, &abs_path))
            .unwrap_or_default();
        let from_probe = probe_res
            .metadata
            .get()
            .map(|m| {
                m.current()
                    .map(|r| SongMetadata::from_revision(r, &abs_path))
                    .unwrap_or_default()
            })
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
                Value::Number(tb.calc_time(*n).seconds.into()),
            );
        }
        if let Ok(size) = fs::metadata(abs_path).map(|meta| meta.len()) {
            data.items.insert(
                TagKey::Extended(ExtendedTagKey::FileSize),
                Value::Number(size.into()),
            );
        }

        Ok(data)
    }

    pub fn get(&self, tag_key: &TagKey) -> Option<&Value> {
        self.items.get(tag_key)
    }

    pub fn matches(&self, filters: &[ParsedFilter]) -> bool {
        filters.iter().all(|filter| {
            self.get(&filter.tag)
                .map(|value| {
                    filter
                        .regex
                        .is_match(value.as_str().unwrap_or(&value.to_string()))
                })
                .is_some_and(|x| x)
        })
    }

    pub fn cover_art(&self) -> Result<String> {
        let mut probe_res = fs_utils::get_probe_result(&self.uri)?;
        let image_src1 = probe_res
            .format
            .metadata()
            .current()
            .and_then(|m| m.visuals().first().cloned());
        let image_src2 = probe_res
            .metadata
            .get()
            .and_then(|m| m.current().and_then(|m| m.visuals().first().cloned()));
        let image = image_src1
            .or(image_src2)
            .ok_or(anyhow!("no cover art found"))?;

        Ok(BASE64_STANDARD.encode(&image.data))
    }

    fn from_revision(revision: &MetadataRevision, uri: &Path) -> Self {
        let mut items = HashMap::new();
        for tag in revision.tags() {
            if let Some(tag_key) = tag.std_key.and_then(|key| TagKey::try_from(key).ok()) {
                items
                    .entry(tag_key)
                    .or_insert_with(|| Value::String(tag.value.to_string()));
            }
        }

        Self {
            items,
            uri: uri.to_path_buf(),
        }
    }

    fn merge(self, other: Self) -> Self {
        let uri = if self.uri.as_os_str().is_empty() {
            other.uri
        } else {
            self.uri
        };

        Self {
            items: self.items.into_iter().chain(other.items).collect(),
            uri,
        }
    }
}
