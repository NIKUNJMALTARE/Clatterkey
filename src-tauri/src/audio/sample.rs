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
}

impl Sample {
    /// `data` is interleaved, one value per channel per frame. A channel
    /// count or sample rate of zero is treated as one.
    pub fn new(channels: u16, sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            data: data.into(),
            channels: channels.max(1),
            sample_rate: sample_rate.max(1),
        }
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

    pub fn duration(&self) -> Duration {
        let frames = self.data.len() / usize::from(self.channels);
        Duration::from_secs_f64(frames as f64 / f64::from(self.sample_rate))
    }

    /// Whether both are the very same buffer in memory, not merely equal.
    pub fn same_buffer(&self, other: &Sample) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}
