use globset::Glob;
use lazy_static::lazy_static;
use std::{env, path::PathBuf};

pub const DEFAULT_PORT: u16 = 2137;
pub const DEFAULT_AUDIO_BACKEND: &str = "default";
pub const DEFAULT_REMOTE_INTERFACE: &str = "0.0.0.0";
pub const DEFAULT_SONG_CACHE_SIZE: usize = 4;
pub const DEFAULT_N_JOBS: usize = 4;

lazy_static! {
    pub static ref DEFAULT_MUSIC_ROOT: PathBuf =
        env::current_dir().expect("foksal cannot access its working directory");
    pub static ref DEFAULT_CONFIG_FILE: PathBuf = dirs::config_local_dir()
        .map(|dir| dir.join("foksal/foksal.toml"))
        .expect("foksal cannot find your config directory");
    pub static ref DEFAULT_LOG_FILE: PathBuf = env::temp_dir().join("foksal.log");
    pub static ref DEFAULT_IGNORE_GLOBSET: Vec<Glob> = Vec::new();
    pub static ref DEFAULT_ALLOWED_EXTS: [String; 3] =
        ["mp3".to_owned(), "m4a".to_owned(), "flac".to_owned()];
}
