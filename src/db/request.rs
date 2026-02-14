use anyhow::{Result, bail};
use serde_json::Value;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf};
use tokio::sync::{mpsc as tokio_chan, oneshot};

use crate::{
    db::{
        core::{Db, SharedDb},
        filter::ParsedFilter,
        tag::TagKey,
    },
    net::{
        core::JsonObject,
        request::{
            DbSubTarget, RawDbRequest, RawMetadataArgs, RawSelectArgs, SubscribeArgs,
            UnsubscribeArgs,
        },
        response::{EventNotif, Response},
    },
};

pub trait ParsedDbRequestArgs {}

pub struct ParsedMetadataArgs {
    pub uris: Vec<PathBuf>,
    pub tags: Vec<TagKey>,
}

pub struct ParsedSelectArgs {
    pub filters: Vec<ParsedFilter>,
    pub group_by: Vec<TagKey>,
}

pub enum DbRequestKind {
    Raw(RawDbRequest),
    Subscribe(SubscribeArgs<DbSubTarget>),
    Unsubscribe(UnsubscribeArgs<DbSubTarget>),
}

pub struct DbRequest {
    pub kind: DbRequestKind,
    pub respond_to: oneshot::Sender<Response>,
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

impl ParsedDbRequestArgs for ParsedSelectArgs {}

impl TryFrom<RawSelectArgs> for ParsedSelectArgs {
    type Error = anyhow::Error;

    fn try_from(raw: RawSelectArgs) -> Result<Self> {
        let filters = match raw
            .filters
            .into_iter()
            .map(ParsedFilter::try_from)
            .collect()
        {
            Ok(filters) => filters,
            Err(e) => bail!(e),
        };
        let group_by = match raw
            .group_by
            .iter()
            .map(|tag_name| TagKey::try_from(tag_name.as_str()))
            .collect()
        {
            Ok(tags) => tags,
            Err(e) => bail!(e),
        };

        Ok(Self { filters, group_by })
    }
}

impl DbRequest {
    pub fn new(kind: DbRequestKind, respond_to: oneshot::Sender<Response>) -> Self {
        Self { kind, respond_to }
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
        let tag_names: Vec<_> = tags.iter().map(|tag| (tag.to_string(), tag)).collect();
        let results: Vec<_> = uris
            .iter()
            .map(|uri| match self.table.get(uri) {
                Some(song_data) => {
                    let pairs: JsonObject = tag_names
                        .iter()
                        .map(|(name, key)| {
                            let value = match song_data.get(key) {
                                Some(v) => Value::String(v.into()),
                                None => Value::Null,
                            };
                            (name.clone(), value)
                        })
                        .collect();
                    Value::Object(pairs)
                }
                None => Value::Null,
            })
            .collect();

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
    pub fn select(&self, ParsedSelectArgs { filters, group_by }: ParsedSelectArgs) -> Response {
        let mut groups: HashMap<Vec<_>, Vec<_>> = HashMap::new();
        for (uri, data) in self.table.iter().filter(|(_, data)| data.matches(&filters)) {
            let ident: Vec<_> = group_by.iter().map(|tag| data.get(tag)).collect();
            groups.entry(ident).or_default().push(uri.to_string_lossy());
        }

        let group_by_tag_names: Vec<_> = group_by.iter().map(|tag| tag.to_string()).collect();
        let values: Vec<_> = groups
            .into_iter()
            .map(|(ident, uris)| {
                let group_data = group_by_tag_names
                    .iter()
                    .cloned()
                    .zip(ident.iter().map(|value| (*value).into()));
                let mut map = JsonObject::from_iter(group_data);
                map.insert("uris".into(), uris.into_iter().collect());

                map
            })
            .collect();

        Response::new_ok().with_item("values", &values)
    }
}

impl SharedDb {
    pub fn add_subscriber(
        &self,
        target: DbSubTarget,
        addr: SocketAddr,
        send_to: tokio_chan::UnboundedSender<EventNotif>,
    ) {
        let mut db = self.inner.write().unwrap();
        db.add_subscriber(target, addr, send_to);
    }

    pub fn remove_subscriber(&self, target: DbSubTarget, addr: SocketAddr) {
        let mut db = self.inner.write().unwrap();
        db.remove_subscriber(target, addr);
    }

    pub fn metadata(&self, args: ParsedMetadataArgs) -> Response {
        let db = self.inner.read().unwrap();
        db.metadata(args)
    }

    pub fn select(&self, args: ParsedSelectArgs) -> Response {
        let db = self.inner.read().unwrap();
        db.select(args)
    }
}
