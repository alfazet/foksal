use anyhow::Result;
use audioadapter_buffers::direct::InterleavedSlice;
use cpal::Sample;
use rubato::{Fft, FixedSync, Resampler};

use libfoksalcommon::{AUDIO_BUF_LEN, CommonSample};

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
        let mut inner = Fft::<CommonSample>::new(
            in_sample_rate,
            out_sample_rate,
            AUDIO_BUF_LEN / n_channels,
            1,
            n_channels,
            FixedSync::Both,
        )?;
        let out_buffer = vec![
            CommonSample::EQUILIBRIUM;
            inner.process_all_needed_output_len(AUDIO_BUF_LEN / n_channels)
                * n_channels
        ];

        Ok(Self { inner, out_buffer })
    }

    /// assume that in_chunk.len() <= AUDIO_BUF_LEN
    pub fn resample(&mut self, in_chunk: &[CommonSample]) -> Result<&[CommonSample]> {
        let n_channels = self.inner.nbr_channels();
        let (in_len, out_len) = (in_chunk.len(), self.out_buffer.len());
        let in_buffer = InterleavedSlice::new(in_chunk, n_channels, in_len / n_channels)?;
        let mut out_buffer =
            InterleavedSlice::new_mut(&mut self.out_buffer, n_channels, out_len / n_channels)?;
        let (_, resampled_len) = self.inner.process_all_into_buffer(
            &in_buffer,
            &mut out_buffer,
            in_len / n_channels,
            None,
        )?;

        Ok(&self.out_buffer[..n_channels * resampled_len])
    }
}
