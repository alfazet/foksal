use anyhow::Result;
use audioadapter_buffers::direct::InterleavedSlice;
use cpal::Sample;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use foksalcommon::CommonSample;

const CHUNK_LEN: usize = 1024;

pub struct ResamplerWrapper {
    inner: Fft<CommonSample>,
    out_buffer: Vec<CommonSample>,
}

impl ResamplerWrapper {
    pub fn try_new(
        in_sample_rate: usize,
        out_sample_rate: usize,
        n_channels: usize,
    ) -> Result<Self> {
        let inner = Fft::<CommonSample>::new(
            in_sample_rate,
            out_sample_rate,
            CHUNK_LEN,
            1,
            n_channels,
            FixedSync::Input,
        )?;
        let out_buffer = vec![CommonSample::EQUILIBRIUM; inner.output_frames_max() * n_channels];

        Ok(Self { inner, out_buffer })
    }

    pub fn input_len(&self) -> usize {
        self.inner.input_frames_next() * self.inner.nbr_channels()
    }

    pub fn resample(&mut self, in_chunk: &[CommonSample]) -> Result<&[CommonSample]> {
        let n_channels = self.inner.nbr_channels();
        let expected_in_frames = self.inner.input_frames_next();
        let in_frames = in_chunk.len() / n_channels;
        let out_frames = self.out_buffer.len() / n_channels;
        let in_buffer = InterleavedSlice::new(in_chunk, n_channels, in_frames)?;
        let mut out_buffer =
            InterleavedSlice::new_mut(&mut self.out_buffer, n_channels, out_frames)?;
        let indexing = if in_frames < expected_in_frames {
            Some(Indexing {
                input_offset: 0,
                output_offset: 0,
                partial_len: Some(in_frames),
                active_channels_mask: None,
            })
        } else {
            None
        };
        let (_, resampled_frames) =
            self.inner
                .process_into_buffer(&in_buffer, &mut out_buffer, indexing.as_ref())?;

        Ok(&self.out_buffer[..n_channels * resampled_frames])
    }
}
