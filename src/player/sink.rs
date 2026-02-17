use anyhow::Result;
use crossbeam::channel as cbeam_chan;
use std::{path::PathBuf, thread};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::{net::request::RawFileRequest, player::request::FileRequest};

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
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    mut rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
) -> Result<()> {
    let mut state = SinkState::Stopped;
    let mut samples = Vec::<CommonSample>::new();
    let mut ts = 0;

    let mut rx_file_request_response: Option<oneshot::Receiver<Bytes>> = None;
    loop {
        let request = match state {
            SinkState::Playing(_) => match rx_sink_request.try_recv() {
                Ok(request) => Some(request),
                Err(cbeam_chan::TryRecvError::Disconnected) => break Ok(()),
                _ => None,
            },
            _ => match rx_sink_request.recv() {
                Ok(request) => Some(request),
                Err(_) => break Ok(()),
            },
        };
        if let Some(request) = request {
            match request {
                SinkRequest::Play(uri) => {
                    println!("requested to play {:?}", uri);
                    let (tx, rx) = oneshot::channel();
                    rx_file_request_response = Some(rx);
                    let _ = tx_file_request.send(FileRequest::new(
                        RawFileRequest::GetChunk {
                            uri: uri.clone(),
                            start: 0,
                            end: 1,
                        },
                        Some(tx),
                    ));

                    state = SinkState::Playing(uri);
                    samples.clear();
                }
                SinkRequest::Pause => {
                    println!("trying to pause");
                    if let SinkState::Playing(uri) = state {
                        println!("paused");
                        state = SinkState::Paused(uri);
                    }
                }
                SinkRequest::Resume => {
                    println!("trying to resume");
                    if let SinkState::Paused(uri) = state {
                        println!("resumed");
                        state = SinkState::Playing(uri);
                    }
                }
                _ => todo!(),
            }
        }
        if let Some(ref mut rx) = rx_file_request_response
            && let Ok(bytes) = rx.try_recv()
        {
            // parse the bytes as an AudioChunk
            println!("got a response: {:?}", bytes);
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
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
) {
    thread::spawn(move || run(tx_file_request, rx_sink_request));
}
