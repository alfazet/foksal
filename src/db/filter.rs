use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use unidecode::unidecode;

use crate::db::tag::TagKey;

// TODO: add some caching

pub struct ParsedFilter {
    pub tag: TagKey,
    pub regex: Regex,
}
