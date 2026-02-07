use anyhow::Result;
use erased_serde::Serialize as ErasedSerialize;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fmt::{self, Display, Formatter},
    path::PathBuf,
};
use tokio::sync::oneshot;

use crate::net::core::JsonObject;

#[derive(Debug, Serialize)]
pub struct Response(JsonObject);

impl Default for Response {
    fn default() -> Self {
        Self(JsonObject::new())
    }
}

impl<T> From<Result<T>> for Response {
    fn from(result: Result<T>) -> Self {
        match result {
            Ok(_) => Self::new_ok(),
            Err(e) => Self::new_err(e.to_string()),
        }
    }
}

impl Response {
    pub fn inner(&self) -> &'_ JsonObject {
        &self.0
    }

    pub fn inner_mut(&mut self) -> &'_ mut JsonObject {
        &mut self.0
    }

    pub fn new_ok() -> Self {
        Self(Map::from_iter([("ok".into(), Value::Bool(true))]))
    }

    pub fn new_err(reason: impl Into<String>) -> Self {
        Self(Map::from_iter([
            ("ok".into(), Value::Bool(false)),
            ("reason".into(), Value::String(reason.into())),
        ]))
    }

    pub fn with_item(mut self, key: impl Into<String>, value: &dyn ErasedSerialize) -> Self {
        let value = match serde_json::to_value(value) {
            Ok(value) => value,
            Err(_) => return self,
        };
        self.inner_mut().insert(key.into(), value);

        self
    }
}
