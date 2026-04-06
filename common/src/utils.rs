use anyhow::Result;
use std::{fs::File, path::Path};
use tracing_subscriber::{Registry, filter, prelude::*};

use crate::config::DEFAULT_LOG_FILE;

pub fn setup_logging(log_file: Option<&Path>) -> Result<()> {
    let log_file = log_file.unwrap_or(DEFAULT_LOG_FILE.as_path());
    let filter = filter::filter_fn(|data| data.target().contains("foksal"));
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(File::create(log_file)?)
        .with_ansi(false)
        .with_line_number(true)
        .with_filter(filter);
    Registry::default().with(layer).init();

    Ok(())
}

/// turns the uri into a valid D-Bus object path
pub fn uri_to_track_id(uri: impl AsRef<Path>, id: usize) -> String {
    let mut safe_uri = format!("/foksal/{}_", id);
    for c in uri.as_ref().to_string_lossy().as_bytes() {
        if c.is_ascii_alphabetic() || c.is_ascii_digit() || *c == b'_' {
            safe_uri.push(*c as char);
        } else {
            safe_uri.push('_');
        }
    }

    safe_uri
}
