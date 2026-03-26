use anyhow::{Result, anyhow, bail};
use clap::Parser;
use globset::Glob;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tracing::{info, warn};

use libfoksalcommon::config::*;

#[derive(Parser)]
#[command(name = "foksal-remote", version, about)]
pub struct RemoteArgs {
    /// Foksal config file
    #[arg(short = 'c', long = "config")]
    pub config_file: Option<PathBuf>,

    /// Address of the interface that foksal should bind to
    #[arg(short = 'i', long = "interface")]
    pub interface: Option<String>,

    /// Port that foksal should listen on
    #[arg(short = 'p', long = "port")]
    pub port: Option<u16>,

    /// Root directory of your music collection
    #[arg(short = 'm', long = "music")]
    pub music_root: Option<PathBuf>,

    /// Foksal log file
    #[arg(short = 'l', long = "log")]
    pub log_file: Option<PathBuf>,
}

#[derive(Default, Deserialize, Serialize)]
struct RawRemoteConfig {
    interface: Option<String>,
    port: Option<u16>,
    music_root: Option<PathBuf>,
    allowed_exts: Option<Vec<String>>,
    ignore_globset: Option<Vec<String>>,
}

pub struct ParsedRemoteConfig {
    pub interface: String,
    pub port: u16,
    pub music_root: PathBuf,
    pub ignore_globset: Vec<Glob>,
    pub allowed_exts: Vec<String>,
}

impl From<&ParsedRemoteConfig> for RawRemoteConfig {
    fn from(parsed: &ParsedRemoteConfig) -> Self {
        let ignore_globset: Vec<_> = parsed
            .ignore_globset
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self {
            interface: Some(parsed.interface.clone()),
            port: Some(parsed.port),
            music_root: Some(parsed.music_root.clone()),
            allowed_exts: Some(parsed.allowed_exts.clone()),
            ignore_globset: Some(ignore_globset),
        }
    }
}

impl ParsedRemoteConfig {
    pub fn try_new(args: RemoteArgs) -> Result<Self> {
        let path = args.config_file.as_ref().unwrap_or(&DEFAULT_CONFIG_FILE);
        if path.exists() && !path.is_file() {
            bail!("`{}` isn't a file", path.to_string_lossy());
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                let raw = toml::from_str::<RawRemoteConfig>(&content)?;
                info!("reading config from `{}`", path.to_string_lossy());

                Ok(Self::try_merge(raw, &args)?)
            }
            Err(e) => {
                warn!(
                    "config file `{}` not found ({}), falling back to default",
                    path.to_string_lossy(),
                    e
                );
                let default_with_cli = Self::try_merge(RawRemoteConfig::default(), &args)?;
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(
                    path,
                    toml::to_string(&RawRemoteConfig::from(&default_with_cli))?,
                )?;
                info!("config file `{}` created", path.to_string_lossy());

                Ok(default_with_cli)
            }
        }
    }

    fn try_merge(raw: RawRemoteConfig, args: &RemoteArgs) -> Result<Self> {
        let ignore_globset = match raw.ignore_globset {
            Some(ignore_globset) => ignore_globset
                .iter()
                .map(|s| Glob::new(s).map_err(|e| anyhow!(e)))
                .collect(),
            None => Ok(DEFAULT_IGNORE_GLOBSET.to_vec()),
        }?;

        Ok(Self {
            interface: args
                .interface
                .clone()
                .unwrap_or(raw.interface.unwrap_or(DEFAULT_REMOTE_INTERFACE.to_owned())),
            port: args.port.unwrap_or(raw.port.unwrap_or(DEFAULT_PORT)),
            music_root: args
                .music_root
                .clone()
                .unwrap_or(raw.music_root.unwrap_or(DEFAULT_MUSIC_ROOT.to_owned())),
            allowed_exts: raw.allowed_exts.unwrap_or(DEFAULT_ALLOWED_EXTS.to_vec()),
            ignore_globset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_args() -> RemoteArgs {
        RemoteArgs {
            config_file: None,
            interface: None,
            port: None,
            music_root: None,
            log_file: None,
        }
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
            port = 2137
            music_root = "/music"
            allowed_exts = ["mp3", "wav"]
            ignore_globset = ["*.tmp", ".*"]
        "#;
        let raw: RawRemoteConfig = toml::from_str(toml).unwrap();
        let parsed = ParsedRemoteConfig::try_merge(raw, &empty_args()).unwrap();

        assert_eq!(parsed.port, 2137);
        assert_eq!(parsed.music_root, PathBuf::from("/music"));
        assert_eq!(parsed.allowed_exts, vec!["mp3", "wav"]);
        assert_eq!(parsed.ignore_globset.len(), 2);
        assert_eq!(parsed.ignore_globset[0].glob(), "*.tmp");
        assert_eq!(parsed.ignore_globset[1].glob(), ".*");
    }

    #[test]
    fn parse_partial_config_uses_defaults() {
        let toml = r#"
            port = 2137
        "#;
        let raw: RawRemoteConfig = toml::from_str(toml).unwrap();
        let parsed = ParsedRemoteConfig::try_merge(raw, &empty_args()).unwrap();

        assert_eq!(parsed.port, 2137);
        assert_eq!(parsed.music_root, *DEFAULT_MUSIC_ROOT);
        assert_eq!(parsed.allowed_exts, DEFAULT_ALLOWED_EXTS.to_vec());
        assert_eq!(parsed.ignore_globset.len(), DEFAULT_IGNORE_GLOBSET.len());
    }

    #[test]
    fn parse_empty_config_uses_all_defaults() {
        let raw: RawRemoteConfig = toml::from_str("").unwrap();
        let parsed = ParsedRemoteConfig::try_merge(raw, &empty_args()).unwrap();

        assert_eq!(parsed.port, DEFAULT_PORT);
        assert_eq!(parsed.music_root, *DEFAULT_MUSIC_ROOT);
        assert_eq!(parsed.allowed_exts, DEFAULT_ALLOWED_EXTS.to_vec());
        assert!(parsed.ignore_globset.is_empty());
    }

    #[test]
    fn cli_args_override_toml_values() {
        let toml = r#"
            port = 2137
            music_root = "/music"
        "#;
        let raw: RawRemoteConfig = toml::from_str(toml).unwrap();
        let args = RemoteArgs {
            config_file: None,
            interface: Some("1.2.3.4".into()),
            port: Some(7312),
            music_root: Some(PathBuf::from("/other")),
            log_file: None,
        };
        let parsed = ParsedRemoteConfig::try_merge(raw, &args).unwrap();

        assert_eq!(parsed.port, 7312);
        assert_eq!(parsed.interface, "1.2.3.4".to_string());
        assert_eq!(parsed.music_root, PathBuf::from("/other"));
    }
}
