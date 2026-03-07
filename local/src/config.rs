use anyhow::{Result, anyhow, bail};
use clap::Parser;
use globset::Glob;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tracing::{info, warn};

use foksalcommon::config::*;

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

#[derive(Default, Deserialize, Serialize)]
struct RawLocalConfig {
    port: Option<u16>,
    music_root: Option<PathBuf>,
    audio_backend: Option<String>,
    allowed_exts: Option<Vec<String>>,
    ignore_globset: Option<Vec<String>>,
}

pub struct ParsedLocalConfig {
    pub port: u16,
    pub music_root: PathBuf,
    pub audio_backend: String,
    pub allowed_exts: Vec<String>,
    pub ignore_globset: Vec<Glob>,
}

impl From<&ParsedLocalConfig> for RawLocalConfig {
    fn from(parsed: &ParsedLocalConfig) -> Self {
        let ignore_globset: Vec<_> = parsed
            .ignore_globset
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self {
            port: Some(parsed.port),
            music_root: Some(parsed.music_root.clone()),
            audio_backend: Some(parsed.audio_backend.clone()),
            allowed_exts: Some(parsed.allowed_exts.clone()),
            ignore_globset: Some(ignore_globset),
        }
    }
}

impl ParsedLocalConfig {
    pub fn try_new(args: LocalArgs) -> Result<Self> {
        let path = args.config_file.as_ref().unwrap_or(&DEFAULT_CONFIG_FILE);
        if path.exists() && !path.is_file() {
            bail!("`{}` isn't a file", path.to_string_lossy());
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                let raw = toml::from_str::<RawLocalConfig>(&content)?;
                info!("reading config from `{}`", path.to_string_lossy());

                Ok(Self::try_merge(raw, &args)?)
            }
            Err(e) => {
                warn!(
                    "config file `{}` not found ({}), falling back to default",
                    path.to_string_lossy(),
                    e
                );
                let default_with_cli = Self::try_merge(RawLocalConfig::default(), &args)?;
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(
                    path,
                    toml::to_string(&RawLocalConfig::from(&default_with_cli))?,
                )?;
                info!("config file `{}` created", path.to_string_lossy());

                Ok(default_with_cli)
            }
        }
    }

    fn try_merge(raw: RawLocalConfig, args: &LocalArgs) -> Result<Self> {
        let ignore_globset = match raw.ignore_globset {
            Some(ignore_globset) => ignore_globset
                .iter()
                .map(|s| Glob::new(s).map_err(|e| anyhow!(e)))
                .collect(),
            None => Ok(DEFAULT_IGNORE_GLOBSET.to_vec()),
        }?;

        Ok(Self {
            port: args.port.unwrap_or(raw.port.unwrap_or(DEFAULT_PORT)),
            music_root: args
                .music_root
                .clone()
                .unwrap_or(raw.music_root.unwrap_or(DEFAULT_MUSIC_ROOT.to_owned())),
            audio_backend: args.audio_backend.clone().unwrap_or(
                raw.audio_backend
                    .unwrap_or(DEFAULT_AUDIO_BACKEND.to_owned()),
            ),
            allowed_exts: raw.allowed_exts.unwrap_or(DEFAULT_ALLOWED_EXTS.to_vec()),
            ignore_globset,
        })
    }
}
