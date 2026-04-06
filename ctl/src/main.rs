use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf};

use libfoksalclient::{
    blocking::BlockingFoksalClient,
    model::{PlayerState, SongMetadata, TagKey, TagValue},
};

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
    /// set playback to loop mode
    Loop,
    /// set playback to random mode
    Random,
    /// set playback to single mode
    Single,
    /// change volume by delta
    VolumeChange { delta: i8 },
    /// set volume
    VolumeSet { volume: u8 },
    /// seek within the current song (by offset)
    SeekBy { seconds: i64 },
    /// seek within the current song (by absolute position)
    SeekTo { seconds: u64 },
    /// clear the playback queue
    Clear,
    /// print the current player state as a JSON object
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

fn convert_metadata(data: &SongMetadata) -> HashMap<String, Value> {
    data.iter()
        .map(|(k, v)| {
            let json_v = match v {
                TagValue::Null => Value::Null,
                TagValue::String(s) => Value::String(s.clone()),
                TagValue::Number(n) => Value::Number((*n).into()),
            };
            (k.to_string(), json_v)
        })
        .collect()
}

fn print_info(state: PlayerState, songs_data: Vec<Option<SongMetadata>>) {
    let data_map: HashMap<_, _> = state.queue.iter().zip(songs_data.iter()).collect();
    let mut output = serde_json::to_value(&state).unwrap();
    let queue: Vec<_> = state
        .queue
        .iter()
        .map(|uri| {
            let metadata = match data_map.get(&uri) {
                Some(Some(data)) => convert_metadata(data),
                _ => HashMap::new(),
            };
            json!({
                "uri": uri,
                "metadata": metadata
            })
        })
        .collect();
    output["queue"] = serde_json::json!(queue);

    // if let Some(uri) = &state.current_song {
    //     let metadata = match data_map.get(uri) {
    //         Some(Some(data)) => convert_metadata(data),
    //         _ => std::collections::HashMap::new(),
    //     };
    //     output["current_song"] = serde_json::json!({
    //         "uri": uri,
    //         "metadata": metadata
    //     });
    // }

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
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
        Command::Loop => {
            client.queue_loop()?;
        }
        Command::Random => {
            client.queue_random()?;
        }
        Command::Single => {
            client.queue_single()?;
        }
        Command::VolumeChange { delta } => {
            client.volume_change(delta)?;
        }
        Command::VolumeSet { volume } => {
            client.volume_set(volume)?;
        }
        Command::SeekTo { seconds } => {
            client.seek_to(seconds)?;
        }
        Command::SeekBy { seconds } => {
            client.seek_by(seconds)?;
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
    let mut client = BlockingFoksalClient::connect("localhost", args.port.unwrap_or(DEFAULT_PORT))?;
    let res = send_request(&mut client, args.command);
    client.close()?;

    res
}
