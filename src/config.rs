use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use globset::GlobSet;
use lazy_static::lazy_static;
use std::{
    env,
    path::{Path, PathBuf},
};

const DEFAULT_PORT: u16 = 2137;
const DEFAULT_MUSIC_ROOT: &str = ".";
const DEFAULT_ALLOWED_EXTS: [&str; 3] = ["mp3", "m4a", "flac"];

lazy_static! {
    static ref DEFAULT_IGNORE_GLOB_SET: GlobSet = GlobSet::empty();
    static ref DEFAULT_LOG_FILE: PathBuf = env::temp_dir().join("foksal.log");
}

#[derive(Subcommand)]
pub enum Mode {
    /// local mode (use this pc's audio and mounted filesystem)
    Local(LocalArgs),
    /// proxy mode (requires connecting to a remore instance)
    Proxy(ProxyArgs),
    /// remote mode (can't play audio, needs to be paired with a proxy)
    Remote(RemoteArgs),
}

#[derive(Parser)]
#[command(version, about, author, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    mode: Mode,
    #[arg(long = "log")]
    log_file: Option<PathBuf>,
}

pub struct CliConfig {
    pub mode: Mode,
    pub log_file: PathBuf,
}

#[derive(Args)]
pub struct LocalArgs {
    /// localhost port for clients to connect to
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,
    /// the root of your music collection
    #[arg(short = 'm', long = "music")]
    music_root: Option<PathBuf>,
}

// TODO: refactor music_root, ignore_glob_set and allowed_exts into some FsConfig in db/fs module
pub struct LocalConfig {
    pub local_port: u16,
    pub music_root: PathBuf,
    pub ignore_glob_set: GlobSet,
    pub allowed_exts: Vec<String>,
}

#[derive(Args)]
pub struct ProxyArgs {
    /// address of the remote instance
    #[arg(short = 'a', long = "addr")]
    remote_addr: String,
    /// port that the remote instance will listen on
    #[arg(long = "hp")]
    remote_port: u16,
    /// port on localhost for clients to connect to
    #[arg(long = "lp")]
    local_port: Option<u16>,
}

pub struct ProxyConfig {
    pub remote_addr: String,
    pub remote_port: u16,
    pub local_port: u16,
}

#[derive(Args)]
pub struct RemoteArgs {
    /// port for proxy instances to connect to
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,
    /// the root of your music collection
    #[arg(short = 'm', long = "music")]
    music_root: Option<PathBuf>,
}

// TODO: refactor music_root, ignore_glob_set and allowed_exts into some FsConfig in db/fs module
pub struct RemoteConfig {
    pub local_port: u16,
    pub music_root: PathBuf,
    pub ignore_glob_set: GlobSet,
    pub allowed_exts: Vec<String>,
}

impl CliConfig {
    // TODO: this should merge with options taken from a config file
    pub fn merge_with_cli(cli_args: CliArgs) -> Self {
        Self {
            mode: cli_args.mode,
            log_file: cli_args.log_file.unwrap_or(DEFAULT_LOG_FILE.clone()),
        }
    }
}

impl LocalConfig {
    pub fn merge_with_cli(cli_args: LocalArgs) -> Self {
        Self {
            local_port: cli_args.port.unwrap_or(DEFAULT_PORT),
            music_root: cli_args.music_root.unwrap_or(DEFAULT_MUSIC_ROOT.into()),
            ignore_glob_set: DEFAULT_IGNORE_GLOB_SET.clone(),
            allowed_exts: DEFAULT_ALLOWED_EXTS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl ProxyConfig {
    pub fn merge_with_cli(cli_args: ProxyArgs) -> Self {
        Self {
            remote_addr: cli_args.remote_addr,
            remote_port: cli_args.remote_port,
            local_port: cli_args.local_port.unwrap_or(DEFAULT_PORT),
        }
    }
}

impl RemoteConfig {
    pub fn merge_with_cli(cli_args: RemoteArgs) -> Self {
        Self {
            local_port: cli_args.port.unwrap_or(DEFAULT_PORT),
            music_root: cli_args.music_root.unwrap_or(DEFAULT_MUSIC_ROOT.into()),
            ignore_glob_set: DEFAULT_IGNORE_GLOB_SET.clone(),
            allowed_exts: DEFAULT_ALLOWED_EXTS.iter().map(|s| s.to_string()).collect(),
        }
    }
}
