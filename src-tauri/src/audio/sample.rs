//! A decoded sound held in memory.

use std::sync::Arc;
use std::time::Duration;

/// Interleaved samples ready to play. Cloning copies a pointer, not the
/// audio, so the key path can hand the same sound out on every press.
#[derive(Debug, Clone)]
pub struct Sample {
    data: Arc<[f32]>,
    channels: u16,
    sample_rate: u32,
    /// How loud to play it: each value is multiplied by this as it is read
    /// for the mixer. The data itself is never changed.
    gain: f32,
    /// How fast to play it: 1.0 is as recorded, above is faster and higher,
    /// below is slower and lower. The real sink reads the buffer at the
    /// sample rate times this, so nothing is resampled here.
    speed: f32,
}

/// A speed that can be played: finite and above zero, or 1.0.
fn usable_speed(speed: f32) -> f32 {
    if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    }
}

impl Sample {
    /// `data` is interleaved, one value per channel per frame. A channel
    /// count or sample rate of zero is treated as one.
    pub fn new(channels: u16, sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            data: data.into(),
            channels: channels.max(1),
            sample_rate: sample_rate.max(1),
            gain: 1.0,
            speed: 1.0,
        }
    }

    /// The same sound at `gain`, sharing the buffer. The gain replaces the
    /// one this sample had; it is not multiplied with it. The speed stays.
    pub fn with_gain(&self, gain: f32) -> Sample {
        self.tuned(gain, self.speed)
    }

    /// The same sound at `gain` and `speed`, sharing the buffer. Both
    /// replace what this sample had. A speed that is not finite or not
    /// above zero is taken as 1.0.
    pub fn tuned(&self, gain: f32, speed: f32) -> Sample {
        Sample {
            data: Arc::clone(&self.data),
            channels: self.channels,
            sample_rate: self.sample_rate,
            gain,
            speed: usable_speed(speed),
        }
    }

    /// 1.0 plays the sound as recorded; 1.1 a tenth faster and higher.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// 1.0 plays the sound as it is; 0.0 is silent.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn samples(&self) -> &[f32] {
        &self.data
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// How long the sound lasts at normal speed.
    pub fn duration(&self) -> Duration {
        let frames = self.data.len() / usize::from(self.channels);
        Duration::from_secs_f64(frames as f64 / f64::from(self.sample_rate))
    }

    /// Whether both are the very same buffer in memory, not merely equal.
    /// Neither the gain nor the speed is compared.
    pub fn same_buffer(&self, other: &Sample) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}
