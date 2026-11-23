//! Decodes an audio file's bytes into a `Sample`, through `rodio`. This and
//! `device` are the only files that name `rodio`; no `rodio` type leaves it.

use std::fmt;
use std::io::Cursor;
use std::time::Duration;

use rodio::decoder::DecoderError;
use rodio::{Decoder, Source};

use super::Sample;

/// Why a file could not become a `Sample`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Not WAV, OGG Vorbis, MP3 or FLAC.
    Unrecognised,
    /// Recognised, and broken.
    Corrupt(String),
    /// Longer than the limit given.
    TooLong,
    /// Decodes to no sound at all.
    Empty,
    /// Every value is zero.
    Silent,
    /// The channel count or sample rate changes part-way through.
    ChangesFormat,
    /// More than two channels: a pack sound is mono or stereo.
    TooManyChannels,
    /// A sample rate above `MAX_SAMPLE_RATE`.
    RateTooHigh,
}

/// The highest sample rate a pack sound may have. Above it a few seconds
/// of sound take a great deal of memory, and nothing is gained.
pub const MAX_SAMPLE_RATE: u32 = 96_000;

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Unrecognised => f.write_str("not a WAV, OGG Vorbis, MP3 or FLAC file"),
            DecodeError::Corrupt(reason) => write!(f, "could not be decoded: {reason}"),
            DecodeError::TooLong => f.write_str("too long"),
            DecodeError::Empty => f.write_str("has no sound in it"),
            DecodeError::Silent => f.write_str("is silent"),
            DecodeError::ChangesFormat => {
                f.write_str("changes channel count or sample rate part-way through")
            }
            DecodeError::TooManyChannels => f.write_str("has more than two channels"),
            DecodeError::RateTooHigh => {
                write!(f, "sample rate above {} kHz", MAX_SAMPLE_RATE / 1000)
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decodes `bytes` fully into memory. The format is found from the content,
/// not a file name. Stops as soon as the sound passes `max`, so a long file
/// is never decoded in full.
pub fn decode(bytes: Vec<u8>, max: Duration) -> Result<Sample, DecodeError> {
    // The byte length lets formats without a length up front (Vorbis, MP3)
    // find their declared end, so encoder padding past it is trimmed.
    let byte_len = bytes.len() as u64;
    let built = Decoder::builder()
        .with_data(Cursor::new(bytes))
        .with_byte_len(byte_len)
        .with_gapless(true)
        .build();
    let mut decoder = built.map_err(|error| match error {
        DecoderError::UnrecognizedFormat | DecoderError::NoStreams => DecodeError::Unrecognised,
        other => DecodeError::Corrupt(other.to_string()),
    })?;

    let channels = decoder.channels().get();
    let sample_rate = decoder.sample_rate().get();
    let declared = decoder.total_duration();
    if channels > 2 {
        return Err(DecodeError::TooManyChannels);
    }
    // Refused before a single value is read.
    if sample_rate > MAX_SAMPLE_RATE {
        return Err(DecodeError::RateTooHigh);
    }
    let max_values =
        (max.as_secs_f64() * f64::from(sample_rate)).ceil() as usize * usize::from(channels);

    let mut data = Vec::new();
    while let Some(value) = decoder.next() {
        if decoder.channels().get() != channels || decoder.sample_rate().get() != sample_rate {
            return Err(DecodeError::ChangesFormat);
        }
        if data.len() >= max_values {
            return Err(DecodeError::TooLong);
        }
        if !value.is_finite() {
            return Err(DecodeError::Corrupt(
                "contains values that are not numbers".into(),
            ));
        }
        data.push(value);
    }

    trim_padding(&mut data, declared, channels, sample_rate);
    if data.len() < usize::from(channels) {
        return Err(DecodeError::Empty);
    }
    if data.iter().all(|value| *value == 0.0) {
        return Err(DecodeError::Silent);
    }
    // A stream that ends part-way through a frame loses the partial frame.
    data.truncate(data.len() - data.len() % usize::from(channels));
    Ok(Sample::new(channels, sample_rate, data))
}

/// Most encoder padding trimmed: about one encoder block. A declared length
/// further from what was decoded is not trusted, so real sound is never cut.
const MAX_PADDING: Duration = Duration::from_millis(100);

/// Some encoders (the Vorbis one in particular) leave padding after the end
/// the file declares, and the decoder does not always drop it. Cut the sound
/// to its declared length when the excess is small enough to be padding.
fn trim_padding(data: &mut Vec<f32>, declared: Option<Duration>, channels: u16, rate: u32) {
    let Some(declared) = declared else {
        return;
    };
    let channels = usize::from(channels);
    let frames = data.len() / channels;
    let declared_frames = (declared.as_secs_f64() * f64::from(rate)).round() as usize;
    let max_padding = (MAX_PADDING.as_secs_f64() * f64::from(rate)) as usize;
    if declared_frames > 0 && declared_frames < frames && frames - declared_frames <= max_padding {
        data.truncate(declared_frames * channels);
    }
}
