use anyhow::Result;
use crossbeam_channel as cbeam_chan;
use rkyv::{access, rancor::Error as RkyvError, util::AlignedVec};
use serde::Serialize;
use std::{path::PathBuf, thread};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;

use crate::{Volume, device::Device, resampler::ResamplerWrapper};
use libfoksalcommon::{
    AUDIO_BUF_LEN, ArchivedAudioChunk, AudioSpec, CommonSample,
    net::request::{FileRequest, RawFileRequest},
};

const REQUEST_SIZE: usize = 8; // multiple of AUDIO_BUF_LEN

#[derive(Clone, Debug, Error, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum SinkError {
    #[error("decoder error ({reason})")]
    Decoder { reason: String },
    #[error("resampler error ({reason})")]
    Resampler { reason: String },
}

pub enum SinkRequest {
    GetState(oneshot::Sender<SinkState>),
    GetCurSong(oneshot::Sender<Option<PathBuf>>),
    GetVolume(oneshot::Sender<Volume>),
    GetElapsed(oneshot::Sender<u64>),
    Play(PathBuf),
    VolChange(i8),
    Seek(isize),
    Stop,
    Pause,
    Resume,
    Toggle,
}

pub enum SinkResponse {
    SongOver,
    StateChanged(SinkState),
    VolumeChanged(Volume),
    Elapsed(u64),
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
    prev_elapsed: u64,
    uri: Option<PathBuf>,
    audio_spec: Option<AudioSpec>,
    resampler: Option<ResamplerWrapper>,
    rx_chunks: Option<oneshot::Receiver<Bytes>>,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SinkState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

struct Sink {
    device: Device,
    state: SinkState,
    data: PlaybackData,
    volume: Volume,
    tx_response: tokio_chan::UnboundedSender<SinkResponse>,
    tx_error: broadcast::Sender<SinkError>,
}

impl SinkError {
    pub fn to_bytes(&self) -> Result<Bytes> {
        let s = serde_json::to_vec(&self)?;
        Ok(s.into())
    }
}

impl Samples {
    fn clear(&mut self) {
        self.inner.clear();
        self.ptr = 0;
        self.got_all = false;
    }
}

impl Sink {
    fn new(
        device: Device,
        tx_response: tokio_chan::UnboundedSender<SinkResponse>,
        tx_error: broadcast::Sender<SinkError>,
    ) -> Self {
        Self {
            device,
            state: Default::default(),
            data: Default::default(),
            volume: Default::default(),
            tx_response,
            tx_error,
        }
    }

    fn change_state(&mut self, new_state: SinkState) {
        self.state = new_state;
        if matches!(new_state, SinkState::Stopped) {
            self.data = Default::default();
        }
        let _ = self.tx_response.send(SinkResponse::StateChanged(new_state));
    }

    fn err_and_stop(&mut self, err: SinkError) {
        let _ = self.tx_error.send(err);
        self.change_state(SinkState::Stopped);
    }

    fn check_song_end(&mut self) {
        let samples = &self.data.samples;
        if samples.ptr >= samples.inner.len() && samples.got_all {
            let _ = self.tx_response.send(SinkResponse::SongOver);
            self.change_state(SinkState::Stopped);
        }
    }

    fn elapsed(&self) -> u64 {
        match self.data.audio_spec {
            Some(AudioSpec {
                n_channels,
                sample_rate,
            }) => self.data.samples.ptr as u64 / (n_channels * sample_rate) as u64,
            None => 0,
        }
    }

    fn recalc_elapsed(&mut self) {
        let new_elapsed = self.elapsed();
        if new_elapsed != self.data.prev_elapsed {
            self.data.prev_elapsed = new_elapsed;
            let _ = self.tx_response.send(SinkResponse::Elapsed(new_elapsed));
        }
    }

    fn seek(&mut self, seconds: isize) {
        let Some(AudioSpec {
            n_channels,
            sample_rate,
        }) = self.data.audio_spec.as_ref()
        else {
            return;
        };
        let offset = (n_channels * sample_rate) as isize * seconds;
        self.data.samples.ptr = self.data.samples.ptr.saturating_add_signed(offset);
        self.check_song_end();
    }

    fn handle_request(&mut self, request: SinkRequest) {
        match request {
            SinkRequest::GetState(respond_to) => {
                let _ = respond_to.send(self.state);
            }
            SinkRequest::GetCurSong(respond_to) => {
                let _ = respond_to.send(self.data.uri.clone());
            }
            SinkRequest::GetVolume(respond_to) => {
                let _ = respond_to.send(self.volume);
            }
            SinkRequest::GetElapsed(respond_to) => {
                let _ = respond_to.send(self.elapsed());
            }
            SinkRequest::VolChange(delta) => {
                self.volume.change(delta);
                let _ = self
                    .tx_response
                    .send(SinkResponse::VolumeChanged(self.volume));
            }
            SinkRequest::Seek(seconds) => {
                self.seek(seconds);
                self.recalc_elapsed();
            }
            SinkRequest::Play(uri) => {
                self.data = Default::default();
                self.data.uri.replace(uri);
                self.data.samples.clear();
                self.change_state(SinkState::Playing);
            }
            SinkRequest::Pause => {
                if let SinkState::Playing = self.state {
                    self.change_state(SinkState::Paused);
                }
            }
            SinkRequest::Resume => {
                if let SinkState::Paused = self.state {
                    self.change_state(SinkState::Playing);
                }
            }
            SinkRequest::Toggle => {
                match self.state {
                    SinkState::Paused => self.change_state(SinkState::Playing),
                    SinkState::Playing => self.change_state(SinkState::Paused),
                    _ => (),
                };
            }
            SinkRequest::Stop => self.change_state(SinkState::Stopped),
        }
    }

    fn request_more_samples(
        &mut self,
        uri: PathBuf,
        tx_file_request: &tokio_chan::UnboundedSender<FileRequest>,
    ) {
        let start = self.data.samples.inner.len();
        let end = start + REQUEST_SIZE * AUDIO_BUF_LEN;
        let (tx, rx) = oneshot::channel();
        let _ = tx_file_request.send(FileRequest::new(
            RawFileRequest::GetChunk { uri, start, end },
            tx,
        ));
        self.data.rx_chunks.replace(rx);
    }

    fn append_samples(&mut self, chunk: &ArchivedAudioChunk) {
        let (n_channels, sample_rate) = (
            chunk.n_channels.to_native() as usize,
            chunk.sample_rate.to_native() as usize,
        );
        if self.data.audio_spec.is_none() {
            self.data.audio_spec = Some(AudioSpec::new(n_channels, sample_rate));
        }
        if self.data.resampler.is_none() {
            match ResamplerWrapper::try_new(sample_rate, self.device.sample_rate(), n_channels) {
                Ok(resampler) => self.data.resampler = Some(resampler),
                Err(e) => {
                    self.err_and_stop(SinkError::Resampler {
                        reason: e.to_string(),
                    });
                    return;
                }
            }
        }
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
            if !self.data.samples.got_all
                && let Some(uri) = self.data.uri.as_ref().cloned()
            {
                match self.data.rx_chunks {
                    Some(ref mut rx) => {
                        if let Ok(bytes) = rx.try_recv() {
                            let mut aligned: AlignedVec = AlignedVec::with_capacity(bytes.len());
                            aligned.extend_from_slice(&bytes);
                            match access::<ArchivedAudioChunk, RkyvError>(&aligned) {
                                Ok(chunk) => {
                                    self.append_samples(chunk);
                                    self.data.rx_chunks = None;
                                }
                                Err(_) => {
                                    self.err_and_stop(SinkError::Decoder {
                                        reason: String::from_utf8_lossy(&bytes).to_string(),
                                    });
                                }
                            }
                        }
                    }
                    None => self.request_more_samples(uri, &tx_file_request),
                }
            }
            if let SinkState::Playing = self.state
                && let Some(ref mut resampler) = self.data.resampler
            {
                if self.data.samples.ptr >= self.data.samples.inner.len() {
                    if self.data.samples.got_all {
                        // seeked past the song's duration
                        self.check_song_end();
                    }
                } else {
                    let samples = &mut self.data.samples;
                    let end =
                        (samples.ptr + resampler.input_len() - 1).min(samples.inner.len() - 1);
                    let buf = &samples.inner[samples.ptr..=end];
                    match resampler.resample(buf) {
                        Ok(resampled) => {
                            let mult = self.volume.to_mult();
                            for sample in resampled {
                                let _ = tx_samples.send(*sample * (mult as CommonSample));
                            }
                            samples.ptr = end + 1;
                            self.recalc_elapsed();
                            self.check_song_end();
                        }
                        Err(e) => {
                            self.err_and_stop(SinkError::Resampler {
                                reason: e.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
}

pub fn spawn_blocking(
    device_name: impl AsRef<str>,
    tx_file_request: tokio_chan::UnboundedSender<FileRequest>,
    rx_sink_request: cbeam_chan::Receiver<SinkRequest>,
    tx_sink_response: tokio_chan::UnboundedSender<SinkResponse>,
    tx_error: broadcast::Sender<SinkError>,
) -> Result<()> {
    let mut device = Device::try_new(device_name).or_else(|_| Device::try_default())?;
    let (tx_samples, rx_samples) = cbeam_chan::bounded(AUDIO_BUF_LEN);
    device.init(rx_samples)?;
    let sink = Sink::new(device, tx_sink_response, tx_error);
    thread::spawn(move || sink.run(tx_samples, tx_file_request, rx_sink_request));

    Ok(())
}
