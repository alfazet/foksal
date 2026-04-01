use anyhow::{Result, anyhow};
use crossbeam_channel::{self as cbeam_chan};
use lru::LruCache;
use rkyv::rancor::Error as RkyvError;
use std::{cmp::Ordering, collections::BinaryHeap, io, num::NonZeroUsize, path::PathBuf};
use symphonia::core::{
    audio::SampleBuffer, codecs::Decoder as SymphoniaDecoder, errors::Error as SymphoniaError,
    formats::FormatReader,
};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;
use tracing::error;

use crate::fs_utils;
use libfoksalcommon::{
    AudioChunk, AudioSpec, CommonSample,
    net::request::{FileRequest, RawFileRequest},
};

struct DecoderRequest {
    start: usize,
    end: usize,
    respond_to: oneshot::Sender<AudioChunk>,
}

enum DecoderState {
    Working,
    Done,
}

enum DecoderResult {
    Ok(AudioSpec),
    Error(anyhow::Error),
    SkipPacket,
    SongOver,
}

pub struct Decoder {
    music_root: PathBuf,
    job_cache: LruCache<PathBuf, cbeam_chan::Sender<DecoderRequest>>,
}

impl PartialEq for DecoderRequest {
    fn eq(&self, other: &Self) -> bool {
        self.end.eq(&other.end)
    }
}

impl Eq for DecoderRequest {}

impl PartialOrd for DecoderRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DecoderRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.end == other.end {
            Ordering::Equal
        } else if self.end < other.end {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

impl Decoder {
    pub fn new(music_root: impl Into<PathBuf>, n_jobs: usize) -> Self {
        let music_root = music_root.into();
        let job_cache = LruCache::new(NonZeroUsize::new(n_jobs).unwrap());

        Self {
            music_root,
            job_cache,
        }
    }

    pub async fn run(mut self, mut rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>) {
        while let Some(FileRequest { raw, respond_to }) = rx_file_request.recv().await {
            match raw {
                RawFileRequest::GetChunk { uri, start, end } => {
                    let bytes = match self.get_chunk(uri, start, end).await {
                        Ok(chunk) => {
                            Bytes::from_owner(rkyv::to_bytes::<RkyvError>(&chunk).unwrap())
                        }
                        Err(e) => Bytes::from(e.to_string().into_bytes()),
                    };
                    let _ = respond_to.send(bytes);
                }
            }
        }
    }

    async fn get_chunk(&mut self, uri: PathBuf, start: usize, end: usize) -> Result<AudioChunk> {
        let (respond_to, rx_response) = oneshot::channel();
        let request = DecoderRequest {
            start,
            end,
            respond_to,
        };
        match self.job_cache.get(&uri) {
            Some(tx) => {
                let _ = tx.send(request);
            }
            None => {
                let tx = self.start_decoding(uri.clone())?;
                let _ = tx.send(request);
                self.job_cache.push(uri, tx);
            }
        }

        Ok(rx_response.await?)
    }

    fn start_decoding(&mut self, uri: PathBuf) -> Result<cbeam_chan::Sender<DecoderRequest>> {
        let probe_res = fs_utils::get_probe_result(fs_utils::to_absolute(&uri, &self.music_root))?;
        let mut demuxer = probe_res.format;
        let track = demuxer.default_track().ok_or(anyhow!(
            "no audio track found in `{}`",
            uri.to_string_lossy()
        ))?;
        let track_id = track.id;
        let mut decoder =
            symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;

        let (tx_request, rx_request) = cbeam_chan::unbounded::<DecoderRequest>();
        tokio::task::spawn_blocking(move || {
            let mut state = DecoderState::Working;
            let mut samples = Vec::<CommonSample>::new();
            let mut pending_requests = BinaryHeap::new();
            let audio_spec = Self::decode_next_packet(
                decoder.as_mut(),
                demuxer.as_mut(),
                &mut samples,
                track_id,
            );
            let AudioSpec {
                n_channels,
                sample_rate,
            } = match audio_spec {
                DecoderResult::Ok(audio_spec) => audio_spec,
                DecoderResult::Error(e) => {
                    error!("{}", e);
                    return;
                }
                _ => {
                    error!("empty file");
                    return;
                }
            };

            loop {
                match state {
                    DecoderState::Working => {
                        if let Ok(request) = rx_request.try_recv() {
                            pending_requests.push(request);
                        }
                        match Self::decode_next_packet(
                            decoder.as_mut(),
                            demuxer.as_mut(),
                            &mut samples,
                            track_id,
                        ) {
                            DecoderResult::Error(e) => {
                                error!("{}", e);
                                break;
                            }
                            DecoderResult::SongOver => {
                                state = DecoderState::Done;
                            }
                            _ => (),
                        }
                        // TODO: rewrite with .pop_if() once it stabilizes
                        while let Some(DecoderRequest { end, .. }) = pending_requests.peek()
                            && *end < samples.len()
                        {
                            let DecoderRequest {
                                start,
                                end,
                                respond_to,
                            } = pending_requests.pop().unwrap();
                            let requested_samples = samples[start..=end].to_vec();
                            let chunk =
                                AudioChunk::new(requested_samples, n_channels, sample_rate, false);
                            let _ = respond_to.send(chunk);
                        }
                    }
                    DecoderState::Done => match rx_request.recv() {
                        Ok(DecoderRequest {
                            start,
                            end,
                            respond_to,
                        }) => {
                            let _ = respond_to.send(AudioChunk::new(
                                samples[start..=end.min(samples.len() - 1)].to_vec(),
                                n_channels,
                                sample_rate,
                                end >= samples.len(),
                            ));
                        }
                        Err(_) => break,
                    },
                }
            }
        });

        Ok(tx_request)
    }

    fn decode_next_packet(
        decoder: &mut dyn SymphoniaDecoder,
        demuxer: &mut dyn FormatReader,
        samples: &mut Vec<CommonSample>,
        track_id: u32,
    ) -> DecoderResult {
        match demuxer.next_packet() {
            Ok(packet) if packet.track_id() == track_id => match decoder.decode(&packet) {
                Ok(data) => {
                    let spec = data.spec();
                    let audio_spec = AudioSpec::new(spec.channels.count(), spec.rate as usize);
                    let mut buf = SampleBuffer::new(data.capacity() as u64, *spec);
                    buf.copy_interleaved_ref(data);
                    samples.extend(buf.samples());

                    DecoderResult::Ok(audio_spec)
                }
                Err(e) => match e {
                    SymphoniaError::ResetRequired
                    | SymphoniaError::DecodeError(_)
                    | SymphoniaError::IoError(_) => DecoderResult::SkipPacket,
                    _ => DecoderResult::Error(anyhow!(e)),
                },
            },
            Err(e) => match e {
                SymphoniaError::ResetRequired => {
                    decoder.reset();
                    DecoderResult::SkipPacket
                }
                SymphoniaError::IoError(e) if matches!(e.kind(), io::ErrorKind::UnexpectedEof) => {
                    // entire song decoded
                    DecoderResult::SongOver
                }
                _ => DecoderResult::Error(anyhow!(e)),
            },
            _ => DecoderResult::SkipPacket,
        }
    }
}
