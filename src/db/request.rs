use anyhow::{Error, Result, bail};
use serde_json::Value;
use std::path::PathBuf;

use crate::{
    db::{
        core::{Db, SharedDb},
        tag::TagKey,
    },
    net::{core::JsonObject, request::RawMetadataArgs, response::Response},
};

pub trait ParsedDbRequestArgs {}

pub struct ParsedMetadataArgs {
    pub uris: Vec<PathBuf>,
    pub tags: Vec<TagKey>,
}

impl ParsedDbRequestArgs for ParsedMetadataArgs {}

impl TryFrom<RawMetadataArgs> for ParsedMetadataArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawMetadataArgs) -> Result<Self> {
        let uris = raw.uris;
        let tags = match raw
            .tags
            .iter()
            .map(|tag_name| TagKey::try_from(tag_name.as_str()))
            .collect()
        {
            Ok(tags) => tags,
            Err(e) => bail!(e),
        };

        Ok(Self { uris, tags })
    }
}

impl Db {
    /// returns the values of requested `tags` from songs in `uris`
    /// response format:
    /// ```json
    /// {
    ///     "ok": true,
    ///     "metadata": [
    ///         {"tag1": "value11", "tag2": "value12", ...},
    ///         {"tag1": "value21", "tag2": "value22", ...},
    ///     ]
    /// }
    /// ```
    pub fn metadata(&self, ParsedMetadataArgs { uris, tags }: ParsedMetadataArgs) -> Response {
        let mut results = Vec::new();
        for uri in uris {
            let uri_result = match self.table.get(&uri) {
                Some(song_data) => {
                    let mut pairs = JsonObject::new();
                    for (tag_name, tag_key) in tags.iter().map(|tag| (tag.to_string(), tag)) {
                        let value = match song_data.get(tag_key) {
                            Some(value) => Value::String(value.into()),
                            None => Value::Null,
                        };
                        pairs.insert(tag_name, value);
                    }

                    Value::Object(pairs)
                }
                None => Value::Null,
            };
            results.push(uri_result);
        }

        Response::new_ok().with_item("metadata", &results)
    }

    /// returns uris of songs that match the given `filters`, grouped by tags in `group_by`
    /// a filter looks like the following: {"tag": "tag_name", "regex": "regex"}
    /// a song must match all provided filters to be included in the response
    /// response format:
    /// ```json
    /// {
    ///     "ok": true,
    ///     "values": [
    ///         {"group_by_tag1": "value11", "group_by_tag2": "value12", ..., "uris": ["uri11", "uri12", ...]},
    ///         {"group_by_tag1": "value21", "group_by_tag2": "value22", ..., "uris": ["uri21", "uri22", ...]},
    ///     ]
    /// }
    /// ```
    pub fn select(&self) -> Response {
        todo!()
    }
}

impl SharedDb {
    pub fn metadata(&self, args: ParsedMetadataArgs) -> Response {
        let db = self.0.read().unwrap();
        db.metadata(args)
    }
}
