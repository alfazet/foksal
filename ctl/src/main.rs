use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{collections::HashMap, path::PathBuf};

use libfoksalclient::{
    blocking::BlockingFoksalClient,
    model::{PlaybackState, PlayerState, QueueMode, SongMetadata, TagKey},
};

const DEFAULT_HOST: &str = "localhost";
const DEFAULT_PORT: u16 = 2137;

#[derive(Subcommand)]
enum Command {
    /// add songs to the end of the playback queue (paths can be relative)
    Add { uris: Vec<PathBuf> },
    /// start playing the song at a given queue position
    Play {
        /// queue position (0-indexed)
        pos: usize,
    },
    /// add songs to the end of the playback queue and play them immediately
    AddPlay { uris: Vec<PathBuf> },
    /// move a song from one queue position to another
    Move { from: usize, to: usize },
    /// remove a song from the queue by position (0-indexed)
    Remove { pos: usize },
    /// pause playback
    Pause,
    /// resume playback
    Resume,
    /// toggle between playing and paused
    Toggle,
    /// stop playback
    Stop,
    /// go to the next song
    Next,
    /// go to the previous song
    Prev,
    /// set playback to sequential mode
    Sequential,
    /// set playback to random mode
    Random,
    /// set playback to loop mode
    Loop,
    /// change volume by delta
    VolumeChange { delta: i8 },
    /// set volume
    VolumeSet { volume: u8 },
    /// seek within the current song
    Seek { seconds: i64 },
    /// clear the playback queue
    Clear,
    /// query the player state
    State,
}

/// foksal-ctl – a basic command line controller for foksal
#[derive(Parser)]
#[command(name = "foksal-ctl", version, about, infer_subcommands = true)]
struct CliArgs {
    /// address of the foksal instance
    #[arg(short = 'h', long, global = true)]
    host: Option<String>,

    /// port that the foksal instance (local or proxy) is listening on
    #[arg(short = 'p', long, global = true)]
    port: Option<u16>,

    #[command(subcommand)]
    command: Command,
}

const TAGS: [TagKey; 4] = [
    TagKey::Artist,
    TagKey::Album,
    TagKey::TrackTitle,
    TagKey::Duration,
];
const N_A: &str = "[n/a]";

fn playback_state_str(state: PlaybackState) -> String {
    let s = match state {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
    };

    format!("state:\t{}", s)
}

fn format_song(data: &SongMetadata) -> String {
    let values: Vec<_> = TAGS
        .iter()
        .map(|tag| {
            data.get(tag)
                .and_then(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_owned())
                    } else {
                        v.as_i64().map(|s| s.to_string())
                    }
                })
                .unwrap_or(N_A.into())
        })
        .collect();

    let mut s = String::new();
    for value in values {
        s.push_str(value.as_str());
        s.push('\t');
    }
    s.pop();

    s
}

fn queue_mode_str(mode: QueueMode) -> String {
    let s = match mode {
        QueueMode::Sequential => "sequential",
        QueueMode::Random => "random",
        QueueMode::Loop => "loop",
    };

    format!("queue_mode:\t{}", s)
}

fn queue_pos_str(pos: Option<usize>) -> String {
    let s = match pos {
        Some(pos) => pos.to_string(),
        None => N_A.into(),
    };

    format!("queue_pos:\t{}", s)
}

fn queue_str(data: &HashMap<&PathBuf, &Option<SongMetadata>>, queue: &[PathBuf]) -> String {
    let mut s = String::new();
    for (i, uri) in queue.iter().enumerate() {
        s.push_str(&format!("\n{}\t", i));
        match *data.get(&uri).unwrap_or(&&Default::default()) {
            Some(song_data) => s.push_str(&format_song(song_data)),
            None => s.push_str(N_A),
        }
    }

    s
}

fn print_info(state: PlayerState, songs_data: Vec<Option<SongMetadata>>) {
    let data_map: HashMap<_, _> = state.queue.iter().zip(songs_data.iter()).collect();
    let current_song_str = match state.current_song {
        Some(uri) => {
            let data = match data_map.get(&uri) {
                Some(Some(data)) => data,
                _ => &Default::default(),
            };

            format_song(data)
        }
        None => N_A.into(),
    };
    let current_song_str = format!("current_song:\t{}", current_song_str);
    let elapsed_str = format!("elapsed:\t{}", state.elapsed);
    let volume_str = format!("volume:\t{}", state.volume);
    let state_str = playback_state_str(state.playback_state);
    let queue_mode_str = queue_mode_str(state.queue_mode);
    let queue_pos_str = queue_pos_str(state.queue_pos);
    let queue_str = queue_str(&data_map, &state.queue);

    println!("{}", current_song_str);
    println!("{}", elapsed_str);
    println!("{}", volume_str);
    println!("{}", state_str);
    println!("{}", queue_mode_str);
    println!("{}", queue_pos_str);
    println!("{}", queue_str);
}

fn send_request(client: &mut BlockingFoksalClient, command: Command) -> Result<()> {
    match command {
        Command::Add { uris } => {
            client.add_to_queue(uris, None)?;
        }
        Command::Play { pos } => {
            client.play(pos)?;
        }
        Command::AddPlay { uris } => {
            client.add_and_play(uris)?;
        }
        Command::Move { from, to } => {
            client.queue_move(from, to)?;
        }
        Command::Remove { pos } => {
            client.remove_from_queue(pos)?;
        }
        Command::Pause => {
            client.pause()?;
        }
        Command::Resume => {
            client.resume()?;
        }
        Command::Toggle => {
            client.toggle()?;
        }
        Command::Stop => {
            client.stop()?;
        }
        Command::Next => {
            client.next()?;
        }
        Command::Prev => {
            client.prev()?;
        }
        Command::Sequential => {
            client.queue_seq()?;
        }
        Command::Random => {
            client.queue_random()?;
        }
        Command::Loop => {
            client.queue_loop()?;
        }
        Command::VolumeChange { delta } => {
            client.volume_change(delta)?;
        }
        Command::VolumeSet { volume } => {
            client.volume_set(volume)?;
        }
        Command::Seek { seconds } => {
            client.seek(seconds)?;
        }
        Command::Clear => {
            client.queue_clear()?;
        }
        Command::State => {
            let state = client.state()?;
            let songs_data = client.metadata(state.queue.clone(), TAGS.to_vec())?;
            print_info(state, songs_data);
        }
    };

    Ok(())
}

fn main() -> Result<()> {
    let args = CliArgs::parse();
    let mut client = BlockingFoksalClient::connect(
        args.host.unwrap_or(DEFAULT_HOST.to_string()),
        args.port.unwrap_or(DEFAULT_PORT),
    )?;
    let res = send_request(&mut client, args.command);
    client.close()?;

    res
}
