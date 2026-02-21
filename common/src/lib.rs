pub mod config;
pub mod net;
pub mod utils;

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

pub type CommonSample = f32;

pub const AUDIO_BUF_LEN: usize = 8192;

#[derive(Clone, Copy)]
pub struct AudioSpec {
    pub n_channels: usize,
    pub sample_rate: usize,
}

#[derive(Archive, Clone, Default, RkyvDeserialize, RkyvSerialize)]
pub struct AudioChunk {
    pub samples: Vec<CommonSample>,
    pub n_channels: usize,
    pub sample_rate: usize,
    pub is_final: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawFilter {
    pub tag: String,
    pub regex: String,
}

impl AudioSpec {
    pub fn new(n_channels: usize, sample_rate: usize) -> Self {
        Self {
            n_channels,
            sample_rate,
        }
    }
}

impl AudioChunk {
    pub fn new(
        samples: Vec<CommonSample>,
        n_channels: usize,
        sample_rate: usize,
        is_final: bool,
    ) -> Self {
        Self {
            samples,
            n_channels,
            sample_rate,
            is_final,
        }
    }

    pub fn slice(&self, start: usize, end: usize) -> Self {
        let end = end.min(self.samples.len() - 1);
        let is_final = end >= self.samples.len();

        Self {
            samples: self.samples[start..=end].to_vec(),
            n_channels: self.n_channels,
            sample_rate: self.sample_rate,
            is_final,
        }
    }
}
