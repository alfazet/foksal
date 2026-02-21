use anyhow::Result;
use std::{fs::File, path::Path};

use crate::config::DEFAULT_LOG_FILE;

pub fn setup_logging(log_file: Option<&Path>) -> Result<()> {
    let log_file = log_file.unwrap_or(DEFAULT_LOG_FILE.as_path());
    tracing_subscriber::fmt()
        .with_writer(File::create(log_file)?)
        .with_ansi(false)
        .with_line_number(true)
        .init();

    Ok(())
}
