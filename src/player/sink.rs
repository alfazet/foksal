use anyhow::Result;
use crossbeam::channel as cbeam_chan;
use rkyv::{access, deserialize, rancor::Error as RkyvError};
use std::{path::PathBuf, thread};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::{
    audio_common::{ArchivedAudioChunk, AudioChunk, CommonSample},
    net::request::RawFileRequest,
    player::request::FileRequest,
};

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
    Playing { uri: PathBuf, ts: u32 },
    Paused { uri: PathBuf, ts: u32 },
    Stopped,
}

fn run(
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
) -> Result<()> {
    let mut state = SinkState::Stopped;
    let mut src_n_channels = None;
    let mut src_sample_rate = None;
    let mut samples = Vec::<CommonSample>::new();

    let mut rx_file_request_response: Option<oneshot::Receiver<Bytes>> = None;
    loop {
        let request = match state {
            SinkState::Playing { .. } => match rx_sink_request.try_recv() {
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
                        tx,
                    ));
                    state = SinkState::Playing { uri, ts: 0 };
                    samples.clear();
                }
                SinkRequest::Pause => {
                    if let SinkState::Playing { uri, ts } = state {
                        state = SinkState::Paused { uri, ts };
                    }
                }
                SinkRequest::Resume => {
                    if let SinkState::Paused { uri, ts } = state {
                        state = SinkState::Playing { uri, ts };
                    }
                }
                _ => todo!(),
            }
        }
        if let Some(ref mut rx) = rx_file_request_response
            && let Ok(bytes) = rx.try_recv()
        {
            let chunk = access::<ArchivedAudioChunk, RkyvError>(&bytes).unwrap();
            src_n_channels.replace(chunk.n_channels);
            src_sample_rate.replace(chunk.sample_rate);
            samples.extend(chunk.samples.as_slice().iter().map(|x| x.to_native()));
            // TODO: also take timestamps from AudioChunks (musing, line 292 in decoder.rs)
        }

        if let SinkState::Playing { uri, ts } = &state {
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
