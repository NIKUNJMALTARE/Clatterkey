//! The real sound device, through `rodio`. This is the only file that names
//! `rodio` or `cpal`; none of their types leave it.

use std::num::NonZero;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::cpal::{self, BufferSize, DeviceId, ErrorKind};
use rodio::mixer::Mixer;
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{ChannelCount, SampleRate, Source};

use super::output::{AudioError, Backend};
use super::{Sample, Sink};

#[cfg(test)]
#[path = "device_tests.rs"]
mod device_tests;

/// The output buffer the app asks for, in frames: 5 ms. The buffer is the
/// main part of the delay between a key press and its click, so it is kept
/// well under the 10 ms the latency budget allows.
pub(super) fn buffer_frames(sample_rate: u32) -> u32 {
    (sample_rate / 200).max(1)
}

/// Plays sounds by adding them to a `rodio` mixer, where each one is its own
/// voice: a new sound never stops or restarts one that is playing.
pub(super) struct MixerSink {
    mixer: Mixer,
}

impl MixerSink {
    pub(super) fn new(mixer: Mixer) -> Self {
        Self { mixer }
    }
}

impl Sink for MixerSink {
    /// Queues the sound for the audio thread and returns. Conversion to the
    /// device's format happens on the audio thread as the sound is read.
    fn play(&self, sample: &Sample) {
        self.mixer.add(SampleSource {
            sample: sample.clone(),
            position: 0,
        });
    }
}

/// Reads a `Sample`'s shared buffer for the mixer, without copying it,
/// applying the sample's gain as it goes.
struct SampleSource {
    sample: Sample,
    position: usize,
}

impl Iterator for SampleSource {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.sample.samples().get(self.position).copied()?;
        self.position += 1;
        Some(value * self.sample.gain())
    }
}

impl Source for SampleSource {
    fn current_span_len(&self) -> Option<usize> {
        // The whole sound is one span; zero tells the mixer it has ended.
        let len = self.sample.samples().len();
        Some(if self.position >= len { 0 } else { len })
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(self.sample.channels()).unwrap_or(NonZero::<u16>::MIN)
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.sample.sample_rate()).unwrap_or(NonZero::<u32>::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.sample.duration())
    }
}

/// An open output stream on one device. Dropping it closes the stream.
pub(super) struct DeviceStream {
    _sink: MixerDeviceSink,
    /// The device it was opened on, when the OS can name it.
    device: Option<DeviceId>,
    /// Set by the stream's error callback when the stream can no longer play.
    failed: Arc<AtomicBool>,
}

/// Opens the system's default output device.
pub(super) struct DeviceBackend {
    /// Wakes the supervisor when a stream fails.
    wake: Sender<()>,
}

impl DeviceBackend {
    pub(super) fn new(wake: Sender<()>) -> Self {
        Self { wake }
    }
}

/// Whether a stream error means the stream must be re-opened. The others
/// report a glitch or a change the stream has already dealt with.
fn is_fatal(kind: ErrorKind) -> bool {
    !matches!(
        kind,
        ErrorKind::Xrun | ErrorKind::RealtimeDenied | ErrorKind::DeviceChanged
    )
}

fn open_error(error: impl std::fmt::Display) -> AudioError {
    AudioError::Open(error.to_string())
}

impl Backend for DeviceBackend {
    type Stream = DeviceStream;

    fn open(&mut self) -> Result<(DeviceStream, Arc<dyn Sink>), AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(AudioError::NoDevice)?;
        let id = device.id().ok();
        let sample_rate = device
            .default_output_config()
            .map_err(open_error)?
            .sample_rate();

        let failed = Arc::new(AtomicBool::new(false));
        let on_error = {
            let failed = Arc::clone(&failed);
            let wake = self.wake.clone();
            // Nothing is printed here: the supervisor re-opens the output.
            move |error: cpal::Error| {
                if is_fatal(error.kind()) {
                    failed.store(true, Ordering::Relaxed);
                    let _ = wake.send(());
                }
            }
        };

        // First with the small buffer, then with the largest the latency
        // budget allows. If the device refuses both, take what it offers:
        // sound with more delay is better than none.
        let small = buffer_frames(sample_rate);
        let mut opened = None;
        for frames in [small, small * 2] {
            let attempt = DeviceSinkBuilder::from_device(device.clone())
                .map_err(open_error)?
                .with_buffer_size(BufferSize::Fixed(frames))
                .with_error_callback(on_error.clone())
                .open_stream();
            if let Ok(sink) = attempt {
                opened = Some(sink);
                break;
            }
        }
        let mut sink = match opened {
            Some(sink) => sink,
            None => DeviceSinkBuilder::from_device(device)
                .map_err(open_error)?
                .with_error_callback(on_error)
                .open_sink_or_fallback()
                .map_err(open_error)?,
        };
        // Re-opening on a device change is routine and writes nothing.
        sink.log_on_drop(false);

        let player = Arc::new(MixerSink::new(sink.mixer().clone()));
        let stream = DeviceStream {
            _sink: sink,
            device: id,
            failed,
        };
        Ok((stream, player))
    }

    fn is_stale(&mut self, stream: &DeviceStream) -> bool {
        if stream.failed.load(Ordering::Relaxed) {
            return true;
        }
        let Some(default) = cpal::default_host().default_output_device() else {
            return true;
        };
        // If either device cannot be named, there is nothing to compare.
        match (&stream.device, default.id().ok()) {
            (Some(opened), Some(now)) => *opened != now,
            _ => false,
        }
    }
}
