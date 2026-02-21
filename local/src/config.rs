use anyhow::Result;
use clap::Parser;
use globset::GlobSet;
use std::path::{Path, PathBuf};

use libfoksalcommon::config::*;

#[derive(Parser)]
pub struct LocalArgs {
    /// Foksal config file
    #[arg(short = 'c', long = "config")]
    pub config_file: Option<PathBuf>,

    /// Port for clients to connect to
    #[arg(short = 'p', long = "port")]
    pub port: Option<u16>,

    /// Root directory of your music collection
    #[arg(short = 'm', long = "music")]
    pub music_root: Option<PathBuf>,

    /// Audio backend to use
    #[arg(short = 'a', long = "audio")]
    pub audio_backend: Option<String>,

    /// Foksal log file
    #[arg(short = 'l', long = "log")]
    pub log_file: Option<PathBuf>,
}

pub struct LocalConfig {
    pub port: u16,
    pub music_root: PathBuf,
    pub audio_backend: String,
    pub ignore_globset: GlobSet,
    pub allowed_exts: Vec<String>,
}

impl LocalConfig {
    pub fn new(args: LocalArgs) -> Result<Self> {
        let path = args
            .config_file
            .unwrap_or(DEFAULT_CONFIG_FILE.to_path_buf());
        let from_file = Self::from_file(path)?;

        let port = args
            .port
            .unwrap_or(from_file.as_ref().map(|c| c.port).unwrap_or(DEFAULT_PORT));
        let music_root = args.music_root.unwrap_or(
            from_file
                .as_ref()
                .map(|c| c.music_root.to_owned())
                .unwrap_or(DEFAULT_MUSIC_ROOT.to_owned()),
        );
        let audio_backend = args.audio_backend.unwrap_or(
            from_file
                .as_ref()
                .map(|c| c.audio_backend.to_owned())
                .unwrap_or(DEFAULT_AUDIO_BACKEND.to_owned()),
        );
        let ignore_globset = from_file
            .as_ref()
            .map(|c| c.ignore_globset.to_owned())
            .unwrap_or(DEFAULT_IGNORE_GLOBSET.to_owned());
        let allowed_exts = from_file
            .as_ref()
            .map(|c| c.allowed_exts.to_owned())
            .unwrap_or(DEFAULT_ALLOWED_EXTS.to_vec());

        Ok(Self {
            port,
            music_root,
            audio_backend,
            ignore_globset,
            allowed_exts,
        })
    }

    /// Ok(None) if the file doesn't exist
    // Ok(Some) if exists and was parsed
    // Err if exists and has errors
    fn from_file(path: impl AsRef<Path>) -> Result<Option<Self>> {
        Ok(None)
    }
}
