use rkyv::{Archive, Deserialize, Serialize, rancor::Error as RkyvError, util::AlignedVec};

pub type CommonSample = f32;

#[derive(Archive, Clone, Default, Deserialize, Serialize)]
pub struct AudioChunk {
    pub samples: Vec<CommonSample>,
    pub n_channels: usize,
    pub sample_rate: usize,
}

impl AudioChunk {
    pub fn new(samples: Vec<CommonSample>, n_channels: usize, sample_rate: usize) -> Self {
        Self {
            samples,
            n_channels,
            sample_rate,
        }
    }
}
