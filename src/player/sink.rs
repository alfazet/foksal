use anyhow::Result;
use std::{path::PathBuf, thread};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::player::request::FileRequest;

type CommonSample = f32;

const CHUNK_LEN: u64 = 1; // in seconds

pub enum SinkRequest {
    Play(PathBuf),
    Stop,
    Pause,
    Resume,
    Toggle,
    // TODO: Seek
}

enum SinkState {
    Playing(PathBuf),
    Paused(PathBuf),
    Stopped,
}

// rkyv
struct AudioChunk {}

fn run(
    mut rx_sink_request: tokio_chan::UnboundedReceiver<SinkRequest>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
) -> Result<()> {
    let mut state = SinkState::Stopped;
    let mut samples = Vec::<CommonSample>::new();
    let mut ts = 0;

    let mut rx_file_request_response: Option<oneshot::Receiver<Bytes>> = None;

    loop {
        if let Some(request) = match state {
            SinkState::Playing(_) => rx_sink_request.try_recv().ok(),
            _ => rx_sink_request.blocking_recv(),
        } {
            match request {
                SinkRequest::Play(uri) => {
                    println!("requested to play {:?}", uri);
                    // TODO: send a PrepareFile request
                    samples.clear();
                }
                _ => todo!(),
            }
        }
        if let Some(ref mut rx) = rx_file_request_response
            && let Ok(bytes) = rx.try_recv()
        {
            // parse the bytes as an AudioChunk
        }

        if let SinkState::Playing(uri) = &state {
            // send a GetChunk request with start = ts, end = ts + CHUNK_LEN
            // parse the returned bytes as a (n_channels, sample_rate, n_samples, samples) struct
            // if n_samples < CHUNK_LEN * sample_rate, then note that this is the end of the file
            // and we shouldn't fire any more GetChunk requests
            // (NOTE: remember the samples vs frames thing)
            // otherwise extend the samples vector
        }
    }
}

pub fn spawn_blocking(
    rx_sink_request: tokio_chan::UnboundedReceiver<SinkRequest>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
) {
    thread::spawn(move || run(rx_sink_request, tx_file_request));
}
