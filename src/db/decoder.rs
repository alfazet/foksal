use anyhow::{Result, anyhow, bail};
use crossbeam::channel::{self as cbeam_chan, TryRecvError};
use rkyv::{Archive, Deserialize, Serialize, rancor::Error as RkyvError, util::AlignedVec};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
};
use symphonia::core::{audio::SampleBuffer, errors::Error as SymphoniaError, units::TimeBase};
use tokio::{
    sync::{mpsc as tokio_chan, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::tungstenite::Bytes;
use tracing::error;

use crate::{
    audio_common::{AudioChunk, CommonSample},
    db::fs_utils,
    net::request::RawFileRequest,
    player::request::FileRequest,
};

type DecoderCache = Arc<RwLock<HashMap<PathBuf, AudioChunk>>>;

const MAX_CACHE_SIZE: usize = 128;

struct DecoderRequest {
    start: usize,
    end: usize,
    respond_to: oneshot::Sender<AudioChunk>,
}

/// run this function only if the file isn't cached
fn decode_file(
    uri: PathBuf,
    cache: DecoderCache,
    rx_decoder_request: cbeam_chan::Receiver<DecoderRequest>,
) -> Result<()> {
    let probe_res = fs_utils::get_probe_result(&uri)?;
    let mut demuxer = probe_res.format;
    let track = demuxer.default_track().ok_or(anyhow!(
        "no audio track found in `{}`",
        uri.to_string_lossy()
    ))?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;

    tokio::task::spawn_blocking(move || {
        let mut samples = Vec::<CommonSample>::new();
        let mut cur_request = None;
        let mut n_channels = None;
        let mut sample_rate = None;

        loop {
            match rx_decoder_request.try_recv() {
                Ok(request) => {
                    cur_request.replace(request);
                }
                Err(TryRecvError::Disconnected) => break,
                _ => (),
            }
            if let (Some(n_channels), Some(sample_rate)) = (n_channels, sample_rate)
                && let Some(request) = cur_request.take()
            {
                let (start_i, end_i) = (
                    request.start * n_channels * sample_rate as usize,
                    request.end * n_channels * sample_rate as usize - 1,
                );
                if end_i < samples.len() {
                    let chunk = AudioChunk::new(
                        samples[start_i..=end_i].to_vec(),
                        n_channels,
                        sample_rate as usize,
                    );
                    let _ = request.respond_to.send(chunk);
                } else {
                    cur_request.replace(request);
                }
            }
            match demuxer.next_packet() {
                Ok(packet) if packet.track_id() == track_id => match decoder.decode(&packet) {
                    Ok(data) => {
                        let spec = data.spec();
                        sample_rate.replace(spec.rate);
                        n_channels.replace(spec.channels.count());
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
                        error!("decoder io error ({})", e);
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
        // respond with whatever remains
        if let (Some(n_channels), Some(sample_rate), Some(request)) =
            (n_channels, sample_rate, cur_request.take())
        {
            let start_i = n_channels * sample_rate as usize * request.start;
            let samples = if start_i < samples.len() {
                samples[start_i..].to_vec()
            } else {
                Vec::new()
            };
            let chunk = AudioChunk::new(samples, n_channels, sample_rate as usize);
            let _ = request.respond_to.send(chunk);
        }

        /*
        if let (Some(n_channels), Some(sample_rate)) = (n_channels, sample_rate) {
            let cache_entry = AudioChunk {
                samples,
                n_channels,
                sample_rate: sample_rate as usize,
            };
            // TODO: write into cache, keep MAX_CACHE_SIZE in mind
        }
        */
    });

    Ok(())
}

async fn run(music_root: PathBuf, mut rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>) {
    let mut cur_uri = None;
    let mut tx_decoder_request = None;
    let cache = DecoderCache::new(RwLock::new(HashMap::new()));

    while let Some(FileRequest { raw, respond_to }) = rx_file_request.recv().await {
        match raw {
            RawFileRequest::GetChunk { uri, start, end } => {
                if start >= end {
                    let _ = respond_to.send(Bytes::new()); // empty => error
                    continue;
                }
                let cache_res = {
                    let cache = cache.read().unwrap();
                    cache.get(&uri).cloned()
                };
                let bytes = match cache_res {
                    Some(AudioChunk {
                        samples,
                        n_channels,
                        sample_rate,
                    }) => {
                        let (start_i, end_i) = (
                            start * n_channels * sample_rate,
                            end * n_channels * sample_rate - 1,
                        );
                        let samples = if end_i < samples.len() {
                            samples[start_i..=end_i].to_vec()
                        } else if start_i < samples.len() {
                            samples[start_i..].to_vec()
                        } else {
                            Vec::new()
                        };
                        let chunk = AudioChunk::new(samples, n_channels, sample_rate);

                        rkyv::to_bytes::<RkyvError>(&chunk).unwrap()
                    }
                    None => {
                        if cur_uri.is_none() || cur_uri.as_ref().is_some_and(|cur| cur != &uri) {
                            let (tx, rx) = cbeam_chan::unbounded();
                            if decode_file(uri.clone(), Arc::clone(&cache), rx).is_err() {
                                let _ = respond_to.send(Bytes::new());
                                continue;
                            }
                            cur_uri = Some(uri);
                            tx_decoder_request = Some(tx);
                        }
                        // at this point, the current file is the requested one
                        let tx_decoder_request = tx_decoder_request.as_ref().unwrap();
                        let (respond_to, rx_response) = oneshot::channel();
                        let _ = tx_decoder_request.send(DecoderRequest {
                            start,
                            end,
                            respond_to,
                        });

                        match rx_response.await {
                            Ok(chunk) => rkyv::to_bytes::<RkyvError>(&chunk).unwrap(),
                            Err(_) => AlignedVec::new(),
                        }
                    }
                };

                let bytes = Bytes::from_owner(bytes);
                let _ = respond_to.send(bytes);
            }
        }
    }
}

pub fn spawn(
    music_root: impl Into<PathBuf>,
    rx_file_request: tokio_chan::UnboundedReceiver<FileRequest>,
) {
    let music_root = music_root.into();
    tokio::spawn(async move {
        run(music_root, rx_file_request).await;
    });
}
