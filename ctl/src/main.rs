use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{collections::HashMap, net::TcpStream};
use tokio_tungstenite::tungstenite::{Message as WsMessage, WebSocket, stream::MaybeTlsStream};

#[derive(Subcommand)]
enum Command {
    /// add songs to the end of the playback queue (comma-separated paths, can be relative)
    Add { songs: String },
    /// remove a song from the queue by position (0-indexed)
    Remove { pos: usize },
    /// start playing the song at a given queue position
    Play {
        /// queue position (0-indexed)
        pos: usize,
    },
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
    Volume { delta: i8 },
    /// seek within the current song
    Seek { seconds: i64 },
    /// clear the playback queue
    Clear,
    /// query the player state
    State,
}

/// foksal-ctl – CLI client to control foksal
#[derive(Parser)]
#[command(name = "cliff", version, about, infer_subcommands = true)]
struct Cli {
    /// port that the foksal instance is listening on
    #[arg(long, default_value_t = 2137, global = true)]
    port: u16,

    #[command(subcommand)]
    command: Command,
}

fn build_request(command: &Command) -> Value {
    match command {
        Command::Add { songs } => {
            let uris: Vec<&str> = songs.split(',').map(|s| s.trim()).collect();
            json!({ "kind": "add_to_queue", "uris": uris })
        }
        Command::Remove { pos } => json!({ "kind": "remove_from_queue", "pos": pos }),
        Command::Play { pos } => json!({ "kind": "play", "pos": pos }),
        Command::Pause => json!({ "kind": "pause" }),
        Command::Resume => json!({ "kind": "resume" }),
        Command::Toggle => json!({ "kind": "toggle" }),
        Command::Stop => json!({ "kind": "stop" }),
        Command::Next => json!({ "kind": "next" }),
        Command::Prev => json!({ "kind": "prev" }),
        Command::Sequential => json!({ "kind": "queue_seq" }),
        Command::Random => json!({ "kind": "queue_random" }),
        Command::Loop => json!({ "kind": "queue_loop" }),
        Command::Volume { delta } => json!({ "kind": "volume", "delta": delta }),
        Command::Seek { seconds } => json!({ "kind": "seek", "seconds": seconds }),
        Command::Clear => json!({ "kind": "queue_clear" }),
        Command::State => json!({ "kind": "state" }),
    }
}

fn send_request(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>, request: &Value) -> Result<Value> {
    let msg = WsMessage::Binary(serde_json::to_vec(request)?.into());
    ws.send(msg)?;
    let response = ws.read()?;
    let body = match &response {
        WsMessage::Binary(data) => data.to_vec(),
        _ => bail!("unexpected response type"),
    };

    Ok(serde_json::from_slice(&body)?)
}

fn format_song(uri: impl AsRef<str>, metadata: Option<&Value>) -> String {
    match metadata {
        Some(metadata) => {
            let artist = metadata
                .get("artist")
                .and_then(|v| v.as_str())
                .unwrap_or("[unknown]");
            let album = metadata
                .get("album")
                .and_then(|v| v.as_str())
                .unwrap_or("[unknown]");
            let title = metadata
                .get("tracktitle")
                .and_then(|v| v.as_str())
                .unwrap_or("[unknown]");
            format!("{} - {} - {}", artist, album, title)
        }
        None => uri.as_ref().to_string(),
    }
}

fn fetch_metadata(
    ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    uris: &[&str],
) -> Result<HashMap<String, Value>> {
    let req = json!({
        "kind": "metadata",
        "uris": uris,
        "tags": ["artist", "tracktitle", "album", "duration"]
    });

    let resp = send_request(ws, &req)?;
    let mut map = HashMap::new();
    if let Some(arr) = resp.get("metadata").and_then(|v| v.as_array()) {
        for (i, meta) in arr.iter().enumerate() {
            if !meta.is_null()
                && let Some(&uri) = uris.get(i)
            {
                map.insert(uri.to_string(), meta.clone());
            }
        }
    }

    Ok(map)
}

fn print_state(response: &Value, metadata: &HashMap<String, Value>) {
    let uri = response.get("current_song").and_then(|v| v.as_str());
    let (song, duration) = match uri {
        Some(uri) => {
            let song_meta = metadata.get(uri);
            let song = format_song(uri, song_meta);
            let duration = song_meta
                .and_then(|m| m.get("duration"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            (song, duration)
        }
        None => ("[none]".into(), 0),
    };
    let state = response
        .get("sink_state")
        .and_then(|v| v.as_str())
        .unwrap_or("[unknown]");
    let mode = response
        .get("queue_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("[unknown]");
    let volume = response
        .get("volume")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "??".into());
    let elapsed = response
        .get("elapsed")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let (e_mins, d_mins) = (elapsed / 60, duration / 60);
    let (e_secs, d_secs) = (elapsed % 60, duration % 60);

    println!("state: {}", state);
    println!("mode: {}", mode);
    println!("song: {}", song);
    println!("elapsed: {}:{:02}/{}:{:02}", e_mins, e_secs, d_mins, d_secs);
    println!("volume: {}", volume);
}

fn print_queue(response: &Value, metadata: &HashMap<String, Value>) {
    let queue_pos = response.get("queue_pos").and_then(|v| v.as_i64());
    if let Some(list) = response.get("queue").and_then(|v| v.as_array()) {
        println!("queue:");
        for (i, uri) in list.iter().enumerate() {
            let uri = uri.as_str().unwrap();
            let display = format_song(uri, metadata.get(uri));
            if queue_pos.is_some_and(|p| p as usize == i) {
                println!("{}*: {}", i, display);
            } else {
                println!("{}: {}", i, display);
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let url = format!("ws://127.0.0.1:{}", cli.port);
    let (mut ws, _) = tokio_tungstenite::tungstenite::connect(&url)?;
    let _ = ws.read()?;

    let request = build_request(&cli.command);
    let response = send_request(&mut ws, &request)?;
    let ok = response
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !ok {
        let reason = response.get("reason").and_then(|v| v.as_str()).unwrap();
        bail!("{}", reason);
    }

    if let Command::State = cli.command {
        let uris: Vec<_> = response
            .get("queue")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        let metadata = fetch_metadata(&mut ws, &uris)?;
        print_state(&response, &metadata);
        print_queue(&response, &metadata);
    }

    Ok(())
}
