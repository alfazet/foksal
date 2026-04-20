pub mod core;
pub mod device;
#[cfg(feature = "mpris")]
pub mod mpris;
pub mod player_controller;
pub mod request;
pub mod sink;

mod queue;
mod resampler;

const MAX_VOLUME: u8 = 100;
const K: f32 = 0.07; // https://www.dr-lex.be/info-stuff/volumecontrols.html

#[derive(Copy, Clone)]
pub struct Volume(pub u8);

impl Default for Volume {
    fn default() -> Self {
        Self(50)
    }
}

impl Volume {
    pub fn change(&mut self, delta: i8) {
        self.0 = self.0.saturating_add_signed(delta).min(MAX_VOLUME)
    }

    pub fn set(&mut self, volume: u8) {
        self.0 = volume.min(MAX_VOLUME)
    }

    pub fn to_mult(&self) -> f32 {
        (((K * (self.0 as f32)).exp() - 1.0) / 1000.0).clamp(0.0, 1.0)
    }
}
