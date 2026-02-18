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

const CHUNK_LEN: usize = 1; // in seconds

pub enum SinkRequest {
    Play(PathBuf),
    Stop,
    Pause,
    Resume,
    Toggle,
    // TODO: Seek
}

enum SinkState {
    Playing { uri: PathBuf, ts: usize },
    Paused { uri: PathBuf, ts: usize },
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
    let mut ptr = 0;

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
                            end: CHUNK_LEN,
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
            && !bytes.is_empty()
        {
            let chunk = access::<ArchivedAudioChunk, RkyvError>(&bytes).unwrap();
            src_n_channels.replace(chunk.n_channels);
            src_sample_rate.replace(chunk.sample_rate);

            // let the_samples: Vec<_> = chunk
            //     .samples
            //     .as_slice()
            //     .iter()
            //     .map(|x| x.to_native())
            //     .collect();
            // println!(
            //     "n_samples: {}, avg: {}",
            //     the_samples.len(),
            //     the_samples.iter().map(|x| x.abs()).sum::<f32>() / the_samples.len() as f32
            // );

            samples.extend(chunk.samples.as_slice().iter().map(|x| x.to_native()));
        }
        if let SinkState::Playing { uri, ts } = &mut state {
            // send samples to the cpal audio callback (backpressured channel)
            // let start = ptr;
            // let end = min(samples.len(), ptr + AUDIO_BUF_LEN - 1) (about 100 ms, the length is given in samples)
            // don't do anything if start >= samples.len()

            // resample samples[start..=end] and send them
            // ptr = end + 1
            // (duration can be calculated from the current value of ptr)
        }
    }
}

pub fn spawn_blocking(
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
) {
    thread::spawn(move || run(tx_file_request, rx_sink_request));
}
