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
