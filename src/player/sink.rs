use anyhow::Result;
use crossbeam_channel as cbeam_chan;
use rkyv::{access, deserialize, rancor::Error as RkyvError};
use std::{
    path::{Path, PathBuf},
    thread,
};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;
use tracing::{error, warn};

use crate::{
    audio_common::{AUDIO_BUF_LEN, ArchivedAudioChunk, AudioChunk, AudioSpec, CommonSample},
    net::request::RawFileRequest,
    player::{device::Device, request::FileRequest},
};

const CHUNK_LEN: usize = 1; // in seconds of audio

pub enum SinkRequest {
    Play(PathBuf),
    Stop,
    Pause,
    Resume,
    Toggle,
    // TODO: Seek
}

#[derive(Debug, Default)]
enum SinkState {
    #[default]
    Stopped,
    Playing {
        ts: usize,
    },
    Paused {
        ts: usize,
    },
}

struct Sink {
    state: SinkState,
    device: Device,
}

impl Sink {
    fn new(device: Device) -> Self {
        Self {
            state: Default::default(),
            device,
        }
    }

    fn run(
        &mut self,
        tx_samples: cbeam_chan::Sender<CommonSample>,
        tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
        rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
    ) {
        let mut samples = Vec::<CommonSample>::new();
        let mut rx_chunks: Option<oneshot::Receiver<Bytes>> = None;
        let mut audio_spec = None;
        let mut got_all_samples = false;
        let mut ptr = 0;
        let mut cur_uri = None;

        loop {
            println!("state: {:?}", self.state);
            let request = match self.state {
                SinkState::Playing { .. } => match rx_sink_request.try_recv() {
                    Ok(request) => Some(request),
                    Err(cbeam_chan::TryRecvError::Disconnected) => break,
                    _ => None,
                },
                _ => match rx_sink_request.recv() {
                    Ok(request) => Some(request),
                    Err(_) => break,
                },
            };
            if let Some(request) = request {
                match request {
                    SinkRequest::Play(uri) => {
                        rx_chunks.replace(request_samples(
                            &tx_file_request,
                            uri.clone(),
                            0,
                            CHUNK_LEN,
                        ));
                        samples.clear();
                        audio_spec = None;
                        got_all_samples = false;
                        ptr = 0;
                        cur_uri.replace(uri);

                        self.state = SinkState::Playing { ts: 0 };
                    }
                    SinkRequest::Pause => {
                        if let SinkState::Playing { ts } = self.state {
                            self.state = SinkState::Paused { ts };
                        }
                    }
                    SinkRequest::Resume => {
                        if let SinkState::Paused { ts } = self.state {
                            self.state = SinkState::Playing { ts };
                        }
                    }
                    _ => todo!(),
                }
            }

            if let SinkState::Playing { ts } = &self.state
                && let Some(audio_spec) = audio_spec
                && (ptr < samples.len() || !got_all_samples)
            {
                let end = (ptr + AUDIO_BUF_LEN - 1).min(samples.len() - 1);
                let buf = &samples[ptr..=end];
                // resample
                for sample in buf {
                    let _ = tx_samples.send(*sample);
                }
                ptr = end + 1;
            }
            if let Some(ref uri) = cur_uri
                && let Some(ref mut rx) = rx_chunks
                && let Ok(bytes) = rx.try_recv()
            {
                if bytes.is_empty() {
                    error!("decoding `{}` failed", uri.to_string_lossy());
                    self.state = SinkState::Stopped;
                    continue;
                }
                let chunk = access::<ArchivedAudioChunk, RkyvError>(&bytes).unwrap();
                let (n_channels, sample_rate) = (
                    chunk.n_channels.to_native() as usize,
                    chunk.sample_rate.to_native() as usize,
                );
                audio_spec.replace(AudioSpec::new(n_channels, sample_rate));
                let new_samples: Vec<_> = chunk
                    .samples
                    .as_slice()
                    .iter()
                    .map(|x| x.to_native())
                    .collect();
                if new_samples.len() < CHUNK_LEN * n_channels * sample_rate {
                    // last batch of samples
                    got_all_samples = true;
                }
                samples.extend(new_samples);
                let last_ts = samples.len() / (n_channels * sample_rate);
                rx_chunks.replace(request_samples(
                    &tx_file_request,
                    uri.clone(),
                    last_ts,
                    last_ts + CHUNK_LEN,
                ));
            }
            if got_all_samples && ptr >= samples.len() {
                self.state = SinkState::Stopped;
            }
        }
    }
}

fn request_samples(
    tx_file_request: &tokio_chan::UnboundedSender<FileRequest>,
    uri: PathBuf,
    start: usize,
    end: usize,
) -> oneshot::Receiver<Bytes> {
    let (tx, rx) = oneshot::channel();
    let _ = tx_file_request.send(FileRequest::new(
        RawFileRequest::GetChunk { uri, start, end },
        tx,
    ));

    rx
}

pub fn spawn_blocking(
    device_name: Option<impl AsRef<str>>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
) -> Result<()> {
    let mut device = match device_name {
        Some(name) => Device::try_new(name).or_else(|_| Device::try_default())?,
        None => Device::try_default()?,
    };
    let (tx_samples, rx_samples) = cbeam_chan::bounded(2 * AUDIO_BUF_LEN);
    device.init(rx_samples)?;
    let mut sink = Sink::new(device);
    thread::spawn(move || sink.run(tx_samples, tx_file_request, rx_sink_request));

    Ok(())
}
