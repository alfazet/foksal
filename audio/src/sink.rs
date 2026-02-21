use anyhow::Result;
use crossbeam_channel as cbeam_chan;
use rkyv::{access, rancor::Error as RkyvError};
use std::{path::PathBuf, thread};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;
use tracing::warn;

use crate::device::Device;
use libfoksalcommon::{
    AUDIO_BUF_LEN, ArchivedAudioChunk, AudioSpec, CommonSample,
    net::request::{FileRequest, RawFileRequest},
};

pub enum SinkRequest {
    Play(PathBuf),
    Stop,
    Pause,
    Resume,
    Toggle,
    // TODO: Seek
}

#[derive(Default)]
struct Samples {
    inner: Vec<CommonSample>,
    ptr: usize,
    got_all: bool,
}

#[derive(Default)]
struct PlaybackData {
    samples: Samples,
    uri: PathBuf,
    audio_spec: Option<AudioSpec>,
    rx_chunks: Option<oneshot::Receiver<Bytes>>,
}

#[derive(Debug, Default)]
enum SinkState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

struct Sink {
    device: Device,
    state: SinkState,
    data: PlaybackData,
}

impl Samples {
    fn clear(&mut self) {
        self.inner.clear();
        self.ptr = 0;
        self.got_all = false;
    }
}

impl Sink {
    fn new(device: Device) -> Self {
        Self {
            device,
            state: Default::default(),
            data: Default::default(),
        }
    }

    fn handle_request(&mut self, request: SinkRequest) {
        match request {
            SinkRequest::Play(uri) => {
                self.state = SinkState::Playing;
                self.data = Default::default();
                self.data.uri = uri;
                self.data.samples.clear();
            }
            SinkRequest::Pause => {
                if let SinkState::Playing = self.state {
                    self.state = SinkState::Paused;
                }
            }
            SinkRequest::Resume => {
                if let SinkState::Paused = self.state {
                    self.state = SinkState::Playing;
                }
            }
            SinkRequest::Stop => self.state = SinkState::Stopped,
            _ => todo!(),
        }
    }

    fn request_more_samples(&mut self, tx_file_request: &tokio_chan::UnboundedSender<FileRequest>) {
        let uri = self.data.uri.clone();
        let start = self.data.samples.inner.len();
        let end = start + 8 * AUDIO_BUF_LEN;
        let (tx, rx) = oneshot::channel();
        let _ = tx_file_request.send(FileRequest::new(
            RawFileRequest::GetChunk { uri, start, end },
            tx,
        ));
        self.data.rx_chunks.replace(rx);
    }

    fn append_samples(&mut self, bytes: Bytes) {
        let chunk = match access::<ArchivedAudioChunk, RkyvError>(&bytes) {
            Ok(chunk) => chunk,
            Err(_) => {
                warn!("corrupted audio chunk");
                return;
            }
        };
        let (n_channels, sample_rate) = (
            chunk.n_channels.to_native() as usize,
            chunk.sample_rate.to_native() as usize,
        );
        self.data
            .audio_spec
            .replace(AudioSpec::new(n_channels, sample_rate));
        let new_samples: Vec<_> = chunk
            .samples
            .as_slice()
            .iter()
            .map(|x| x.to_native())
            .collect();
        self.data.samples.inner.extend(new_samples);
        if chunk.is_final {
            self.data.samples.got_all = true;
        }
    }

    fn run(
        mut self,
        tx_samples: cbeam_chan::Sender<CommonSample>,
        tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
        rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
    ) {
        loop {
            let request = match self.state {
                SinkState::Playing => match rx_sink_request.try_recv() {
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
                self.handle_request(request);
            }
            if !self.data.samples.got_all {
                match self.data.rx_chunks {
                    Some(ref mut rx) => {
                        if let Ok(bytes) = rx.try_recv() {
                            if !bytes.is_empty() {
                                self.append_samples(bytes);
                            }
                            self.data.rx_chunks = None;
                        }
                    }
                    None => self.request_more_samples(&tx_file_request),
                }
            }
            if let SinkState::Playing = self.state
                && let Some(audio_spec) = self.data.audio_spec
                && self.data.samples.ptr < self.data.samples.inner.len()
            {
                let samples = &mut self.data.samples;
                let end = (samples.ptr + AUDIO_BUF_LEN - 1).min(samples.inner.len() - 1);
                let buf = &samples.inner[samples.ptr..=end];
                // TODO: resample the buffer
                for sample in buf {
                    let _ = tx_samples.send(*sample);
                }
                samples.ptr = end + 1;
                if samples.ptr >= samples.inner.len() {
                    // TODO: send a message to the player
                    self.state = SinkState::Stopped;
                }
            }
        }
    }
}

pub fn spawn_blocking(
    device_name: impl AsRef<str>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
) -> Result<()> {
    let mut device = Device::try_new(device_name).or_else(|_| Device::try_default())?;
    let (tx_samples, rx_samples) = cbeam_chan::bounded(AUDIO_BUF_LEN);
    device.init(rx_samples)?;
    let sink = Sink::new(device);
    thread::spawn(move || sink.run(tx_samples, tx_file_request, rx_sink_request));

    Ok(())
}
