use anyhow::{Result, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tracing::{info, warn};

use libfoksalcommon::config::*;

#[derive(Parser)]
#[command(name = "foksal-proxy", version, about)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_args() -> ProxyArgs {
        ProxyArgs {
            config_file: None,
            remote_addr: None,
            remote_port: None,
            local_port: None,
            audio_backend: None,
            log_file: None,
        }
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
            remote_addr = "12.34.56.78"
            remote_port = 7312
            local_port = 2137
            audio_backend = "default"
        "#;
        let raw: RawProxyConfig = toml::from_str(toml).unwrap();
        let parsed = ParsedProxyConfig::try_merge(raw, &empty_args()).unwrap();

        assert_eq!(parsed.remote_addr, "12.34.56.78");
        assert_eq!(parsed.remote_port, 7312);
        assert_eq!(parsed.local_port, 2137);
        assert_eq!(parsed.audio_backend, "default");
    }

    #[test]
    fn parse_partial_config_uses_defaults() {
        let toml = r#"
            remote_addr = "12.34.56.78"
        "#;
        let raw: RawProxyConfig = toml::from_str(toml).unwrap();
        let parsed = ParsedProxyConfig::try_merge(raw, &empty_args()).unwrap();

        assert_eq!(parsed.remote_addr, "12.34.56.78");
        assert_eq!(parsed.remote_port, DEFAULT_PORT);
        assert_eq!(parsed.local_port, DEFAULT_PORT);
        assert_eq!(parsed.audio_backend, DEFAULT_AUDIO_BACKEND);
    }

    #[test]
    fn missing_remote_addr_returns_error() {
        let raw: RawProxyConfig = toml::from_str("").unwrap();
        let result = ParsedProxyConfig::try_merge(raw, &empty_args());

        match result {
            Ok(_) => panic!("expected an error when remote_addr is missing"),
            Err(e) => assert!(
                e.to_string().contains("remote"),
                "error message should mention remote address, got: {}",
                e
            ),
        }
    }

    #[test]
    fn cli_args_override_toml_values() {
        let toml = r#"
            remote_addr = "12.34.56.78"
            remote_port = 2137
            local_port = 7312
            audio_backend = "default"
        "#;
        let raw: RawProxyConfig = toml::from_str(toml).unwrap();
        let args = ProxyArgs {
            config_file: None,
            remote_addr: Some("192.168.0.1".to_owned()),
            remote_port: Some(1111),
            local_port: Some(2222),
            audio_backend: Some("pulse".to_owned()),
            log_file: None,
        };
        let parsed = ParsedProxyConfig::try_merge(raw, &args).unwrap();

        assert_eq!(parsed.remote_addr, "192.168.0.1");
        assert_eq!(parsed.remote_port, 1111);
        assert_eq!(parsed.local_port, 2222);
        assert_eq!(parsed.audio_backend, "pulse");
    }
}
