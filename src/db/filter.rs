use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use unidecode::unidecode;

use crate::db::tag::TagKey;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawFilter {
    pub tag: String,
    pub regex: String,
}

#[derive(Debug)]
pub struct ParsedFilter {
    pub tag: TagKey,
    pub regex: Regex,
}

impl TryFrom<RawFilter> for ParsedFilter {
    type Error = anyhow::Error;

    fn try_from(raw: RawFilter) -> Result<Self> {
        let tag = raw.tag.as_str().try_into()?;
        let regex = Regex::new(&unidecode(&raw.regex))?;

        Ok(Self { tag, regex })
    }
}
