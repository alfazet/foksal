use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};

use libfoksalcommon::config::*;

#[derive(Parser)]
pub struct ProxyArgs {
    /// Foksal config file
    #[arg(short = 'c', long = "config")]
    pub config_file: Option<PathBuf>,

    /// Address of the remote instance
    pub remote_addr: String,

    /// Port that the remote insntance is listening on
    #[arg(short = 'r', long = "remote-port")]
    pub remote_port: Option<u16>,

    /// Local port for clients to connect to
    #[arg(short = 'p', long = "local-port")]
    pub local_port: Option<u16>,

    /// Audio backend to use
    #[arg(short = 'a', long = "audio")]
    pub audio_backend: Option<String>,

    /// Foksal log file
    #[arg(short = 'l', long = "log")]
    pub log_file: Option<PathBuf>,
}

pub struct ProxyConfig {
    pub remote_addr: String,
    pub remote_port: u16,
    pub local_port: u16,
    pub audio_backend: String,
}

impl ProxyConfig {
    pub fn new(args: ProxyArgs) -> Result<Self> {
        let path = args
            .config_file
            .unwrap_or(DEFAULT_CONFIG_FILE.to_path_buf());
        let from_file = Self::from_file(path)?;

        let remote_addr = args.remote_addr;
        let remote_port = args.remote_port.unwrap_or(
            from_file
                .as_ref()
                .map(|c| c.remote_port)
                .unwrap_or(DEFAULT_PORT),
        );
        let local_port = args.local_port.unwrap_or(
            from_file
                .as_ref()
                .map(|c| c.local_port)
                .unwrap_or(DEFAULT_PORT),
        );
        let audio_backend = args.audio_backend.unwrap_or(
            from_file
                .as_ref()
                .map(|c| c.audio_backend.to_owned())
                .unwrap_or(DEFAULT_AUDIO_BACKEND.to_owned()),
        );

        Ok(Self {
            remote_addr,
            remote_port,
            local_port,
            audio_backend,
        })
    }

    /// Ok(None) if the file doesn't exist
    // Ok(Some) if exists and was parsed
    // Err if exists and has errors
    fn from_file(path: impl AsRef<Path>) -> Result<Option<Self>> {
        Ok(None)
    }
}
