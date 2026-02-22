use anyhow::{Result, anyhow};
use crossbeam_channel::{self as cbeam_chan, TryRecvError};
use lru::LruCache;
use rkyv::rancor::Error as RkyvError;
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    io,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use symphonia::core::{audio::SampleBuffer, errors::Error as SymphoniaError};
use tokio::sync::{mpsc as tokio_chan, oneshot};
use tokio_tungstenite::tungstenite::Bytes;
use tracing::error;

use crate::fs_utils;
use libfoksalcommon::{
    AudioChunk, AudioSpec, CommonSample,
    net::request::{FileRequest, RawFileRequest},
};

const MAX_CACHE_SIZE: usize = 8; // ~1GB, assuming 5-minute songs at 48kHz stereo
const MAX_N_JOBS: usize = 8;

struct DecoderRequest {
    start: usize,
    end: usize,
    respond_to: oneshot::Sender<AudioChunk>,
}

pub struct Decoder {
    music_root: PathBuf,
    cache: Arc<RwLock<LruCache<PathBuf, AudioChunk>>>,
    jobs: LruCache<PathBuf, cbeam_chan::Sender<DecoderRequest>>,
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
    pub fn new(music_root: impl Into<PathBuf>) -> Self {
        let music_root = music_root.into();
        let cache = Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(MAX_CACHE_SIZE).unwrap(),
        )));
        let jobs = LruCache::new(NonZeroUsize::new(MAX_N_JOBS).unwrap());

        Self {
            music_root,
            cache,
            jobs,
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
                        Err(e) => {
                            error!("decoder error ({})", e);
                            Bytes::new()
                        }
                    };
                    let _ = respond_to.send(bytes);
                }
            }
        }
    }

    async fn get_chunk(&mut self, uri: PathBuf, start: usize, end: usize) -> Result<AudioChunk> {
        if let Some(chunk) = self
            .cache
            .write()
            .unwrap()
            .get(&uri)
            .map(|chunk| chunk.slice(start, end))
        {
            return Ok(chunk);
        }
        let (respond_to, rx_response) = oneshot::channel();
        let request = DecoderRequest {
            start,
            end,
            respond_to,
        };
        match self.jobs.get(&uri) {
            Some(tx) => {
                let _ = tx.send(request);
            }
            None => {
                let tx = self.start_decoding(uri.clone())?;
                let _ = tx.send(request);
                self.jobs.put(uri, tx);
            }
        }

        Ok(rx_response.await?)
    }

    fn start_decoding(&mut self, uri: PathBuf) -> Result<cbeam_chan::Sender<DecoderRequest>> {
        let probe_res = fs_utils::get_probe_result(&uri)?;
        let mut demuxer = probe_res.format;
        let track = demuxer.default_track().ok_or(anyhow!(
            "no audio track found in `{}`",
            uri.to_string_lossy()
        ))?;
        let track_id = track.id;
        let mut decoder =
            symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;

        let cache = Arc::clone(&self.cache);
        let (tx_request, rx_request) = cbeam_chan::unbounded::<DecoderRequest>();
        tokio::task::spawn_blocking(move || {
            let mut samples = Vec::<CommonSample>::new();
            let mut audio_spec = None;
            let mut requests = BinaryHeap::new();
            loop {
                match rx_request.try_recv() {
                    Ok(request) => requests.push(request),
                    Err(TryRecvError::Disconnected) => break,
                    _ => (),
                }
                if let Some(AudioSpec {
                    n_channels,
                    sample_rate,
                }) = audio_spec
                {
                    while let Some(DecoderRequest {
                        start,
                        end,
                        respond_to,
                    }) = requests.pop()
                        && end < samples.len()
                    {
                        let requested_samples = samples[start..=end].to_vec();
                        let chunk =
                            AudioChunk::new(requested_samples, n_channels, sample_rate, false);
                        let _ = respond_to.send(chunk);
                    }
                }
                match demuxer.next_packet() {
                    Ok(packet) if packet.track_id() == track_id => match decoder.decode(&packet) {
                        Ok(data) => {
                            let spec = data.spec();
                            if audio_spec.is_none() {
                                audio_spec =
                                    Some(AudioSpec::new(spec.channels.count(), spec.rate as usize));
                            }
                            let mut buf = SampleBuffer::new(data.capacity() as u64, *spec);
                            buf.copy_interleaved_ref(data);
                            samples.extend(buf.samples());
                        }
                        Err(e) => match e {
                            SymphoniaError::ResetRequired
                            | SymphoniaError::DecodeError(_)
                            | SymphoniaError::IoError(_) => (),
                            _ => {
                                error!("{}", e);
                                break;
                            }
                        },
                    },
                    Err(e) => match e {
                        SymphoniaError::ResetRequired => {
                            decoder.reset();
                        }
                        SymphoniaError::IoError(e)
                            if matches!(e.kind(), io::ErrorKind::UnexpectedEof) =>
                        {
                            // entire song decoded
                            break;
                        }
                        _ => {
                            error!("{}", e);
                            break;
                        }
                    },
                    _ => (),
                }
            }
            if let Some(AudioSpec {
                n_channels,
                sample_rate,
            }) = audio_spec
            {
                while let Some(DecoderRequest {
                    start,
                    end,
                    respond_to,
                }) = requests.pop()
                {
                    let end = end.min(samples.len() - 1);
                    let requested_samples = samples[start..=end].to_vec();
                    let chunk = AudioChunk::new(
                        requested_samples,
                        n_channels,
                        sample_rate,
                        end >= samples.len(),
                    );
                    let _ = respond_to.send(chunk);
                }
                let entire_chunk = AudioChunk::new(samples, n_channels, sample_rate, false);
                let mut cache = cache.write().unwrap();
                cache.put(uri, entire_chunk);
            }
        });

        Ok(tx_request)
    }
}
