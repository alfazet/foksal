use anyhow::{Result, anyhow, bail};
use cpal::{
    Device as CpalDevice, FromSample, OutputCallbackInfo, SampleFormat, SizedSample,
    SupportedStreamConfig,
    platform::Stream as CpalStream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel as cbeam_chan;
use tracing::error;

use crate::audio_common::CommonSample;

trait Sample: FromSample<CommonSample> + SizedSample + Send + 'static {}

impl Sample for i8 {}
impl Sample for i16 {}
impl Sample for i32 {}
impl Sample for i64 {}
impl Sample for u8 {}
impl Sample for u16 {}
impl Sample for u32 {}
impl Sample for u64 {}
impl Sample for f32 {}
impl Sample for f64 {}

pub struct Device {
    inner: CpalDevice,
    config: SupportedStreamConfig,
    stream: Option<CpalStream>,
}

impl TryFrom<CpalDevice> for Device {
    type Error = anyhow::Error;

    fn try_from(inner: CpalDevice) -> Result<Self> {
        let config = inner.default_output_config()?;
        Ok(Self {
            inner,
            config,
            stream: None,
        })
    }
}

impl Device {
    pub fn try_new(name: impl AsRef<str>) -> Result<Self> {
        let host = cpal::default_host();
        let inner = match host.output_devices()?.find(|x| {
            x.description()
                .ok()
                .map(|x| x.name() == name.as_ref())
                .unwrap_or(false)
        }) {
            Some(device) => device,
            None => {
                let mut err_msg = format!(
                    "audio backend `{}` unavailable, available backends: ",
                    name.as_ref()
                );
                for name in host
                    .output_devices()?
                    .filter_map(|x| x.description().ok().map(|x| x.name().to_owned()))
                {
                    err_msg.push_str(&name);
                    err_msg.push_str(", ");
                }
                bail!(err_msg)
            }
        };

        Self::try_from(inner)
    }

    pub fn try_default() -> Result<Self> {
        let host = cpal::default_host();
        let inner = host
            .default_output_device()
            .ok_or(anyhow!("default audio backend not found"))?;

        Self::try_from(inner)
    }

    pub fn n_channels(&self) -> usize {
        self.config.channels() as usize
    }

    pub fn sample_rate(&self) -> usize {
        self.config.sample_rate() as usize
    }

    pub fn init(&mut self, rx_samples: cbeam_chan::Receiver<CommonSample>) -> Result<()> {
        macro_rules! build_output_stream {
            ($type:ty) => {
                self.inner.build_output_stream(
                    &self.config.clone().into(),
                    self.create_data_callback::<$type>(rx_samples)?,
                    |e| error!("playback error ({})", e),
                    None,
                )?
            };
        }

        use SampleFormat as S;
        let stream = match self.config.sample_format() {
            S::I8 => build_output_stream!(i8),
            S::I16 => build_output_stream!(i16),
            S::I32 => build_output_stream!(i32),
            S::I64 => build_output_stream!(i64),
            S::U8 => build_output_stream!(u8),
            S::U16 => build_output_stream!(u16),
            S::U32 => build_output_stream!(u32),
            S::U64 => build_output_stream!(u64),
            S::F32 => build_output_stream!(f32),
            S::F64 => build_output_stream!(f64),
            x => bail!(format!("unsupported sample format `{:?}`", x)),
        };
        stream.play()?;
        self.stream.replace(stream);

        Ok(())
    }

    fn create_data_callback<T>(
        &self,
        rx_sample: cbeam_chan::Receiver<CommonSample>,
    ) -> Result<impl FnMut(&mut [T], &OutputCallbackInfo) + Send + 'static>
    where
        T: Sample,
    {
        let callback = move |data: &mut [T], _: &OutputCallbackInfo| {
            let mut i = 0;
            while let Ok(s) = rx_sample.try_recv() {
                data[i] = T::from_sample(s);
                i += 1;
                if i >= data.len() {
                    break;
                }
            }
            data[i..].fill(T::EQUILIBRIUM);
        };

        Ok(callback)
    }
}
