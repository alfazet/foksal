use anyhow::{Result, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tracing::{info, warn};

use libfoksalcommon::config::*;

#[derive(Parser)]
pub struct ProxyArgs {
    /// Foksal config file
    #[arg(short = 'c', long = "config")]
    pub config_file: Option<PathBuf>,

    /// Address of the remote instance
    #[arg(long = "remote-addr")]
    pub remote_addr: Option<String>,

    /// Port that the remote insntance is listening on
    #[arg(long = "remote-port")]
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

#[derive(Default, Deserialize, Serialize)]
struct RawProxyConfig {
    remote_addr: Option<String>,
    remote_port: Option<u16>,
    local_port: Option<u16>,
    audio_backend: Option<String>,
}

pub struct ParsedProxyConfig {
    pub remote_addr: String,
    pub remote_port: u16,
    pub local_port: u16,
    pub audio_backend: String,
}

impl From<&ParsedProxyConfig> for RawProxyConfig {
    fn from(parsed: &ParsedProxyConfig) -> Self {
        Self {
            remote_addr: Some(parsed.remote_addr.clone()),
            remote_port: Some(parsed.remote_port),
            local_port: Some(parsed.local_port),
            audio_backend: Some(parsed.audio_backend.clone()),
        }
    }
}

impl ParsedProxyConfig {
    pub fn try_new(args: ProxyArgs) -> Result<Self> {
        let path = args.config_file.as_ref().unwrap_or(&DEFAULT_CONFIG_FILE);
        if path.exists() && !path.is_file() {
            bail!("`{}` isn't a file", path.to_string_lossy());
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                let raw = toml::from_str::<RawProxyConfig>(&content)?;
                info!("reading config from `{}`", path.to_string_lossy());

                Ok(Self::try_merge(raw, &args)?)
            }
            Err(e) => {
                warn!(
                    "config file `{}` not found ({}), falling back to default",
                    path.to_string_lossy(),
                    e
                );
                let default_with_cli = Self::try_merge(RawProxyConfig::default(), &args)?;
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(
                    path,
                    toml::to_string(&RawProxyConfig::from(&default_with_cli))?,
                )?;
                info!("config file `{}` created", path.to_string_lossy());

                Ok(default_with_cli)
            }
        }
    }

    fn try_merge(raw: RawProxyConfig, args: &ProxyArgs) -> Result<Self> {
        let remote_addr = match &args.remote_addr {
            Some(remote_addr) => remote_addr.to_owned(),
            None => match raw.remote_addr {
                Some(remote_addr) => remote_addr,
                None => bail!("IP address of the remote must be specified"),
            },
        };

        Ok(Self {
            remote_addr,
            remote_port: args
                .remote_port
                .unwrap_or(raw.remote_port.unwrap_or(DEFAULT_PORT)),
            local_port: args
                .local_port
                .unwrap_or(raw.local_port.unwrap_or(DEFAULT_PORT)),
            audio_backend: args.audio_backend.clone().unwrap_or(
                raw.audio_backend
                    .unwrap_or(DEFAULT_AUDIO_BACKEND.to_owned()),
            ),
        })
    }
}
