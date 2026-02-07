use anyhow::Result;
use serde_json::Value;

use crate::{
    db::{
        core::{Db, SharedDb},
        tag::TagKey,
    },
    net::{
        core::JsonObject,
        request::{MetadataArgs, ParsedRequest},
        response::Response,
    },
};

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
    pub fn metadata(&self, MetadataArgs { uris, tags }: MetadataArgs) -> Response {
        let mut results = Vec::new();
        for uri in uris {
            let uri_result = match self.table.get(&uri) {
                Some(song_data) => {
                    let mut pairs = JsonObject::new();
                    for (tag_name, tag_key) in
                        tags.iter().map(|tag| (tag, TagKey::try_from(tag.as_str())))
                    {
                        match tag_key {
                            Ok(tag_key) => match song_data.get(&tag_key) {
                                Some(value) => {
                                    pairs.insert(tag_name.into(), Value::String(value.into()))
                                }
                                None => pairs.insert(tag_name.into(), Value::Null),
                            },
                            Err(_) => pairs.insert(tag_name.into(), Value::Null),
                        };
                    }

                    Value::Object(pairs)
                }
                None => Value::Null,
            };
            results.push(uri_result);
        }

        Response::new_ok().with_item("metadata", &results)
    }
}

impl SharedDb {
    pub fn metadata(&self, args: MetadataArgs) -> Response {
        let db = self.0.read().unwrap();
        db.metadata(args)
    }
}
