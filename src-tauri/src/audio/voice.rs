//! Chooses the sound for a key of a group going down or up, as the live
//! settings say now. The key path and the preview both use it, so they can
//! never differ. It takes no lock, reads no file and copies no audio.

use super::{Live, Sample, SoundSet};
use crate::keys::{KeyGroup, KeyKind};

/// How fast to play a sound: 1.0 with no variation, otherwise evenly
/// between `1 - variation` and `1 + variation`, by `random`.
pub fn speed_for(variation: f32, random: u32) -> f32 {
    if variation <= 0.0 || !variation.is_finite() {
        return 1.0;
    }
    // The top 24 bits, which an `f32` holds exactly: 0.0 to just under 1.0.
    let unit = (random >> 8) as f32 / (1u32 << 24) as f32;
    1.0 + variation * (2.0 * unit - 1.0)
}

/// The sound to play, with its gain and speed, or `None` if this plays
/// nothing. It does not look at whether sound is on: the caller decides.
///
/// A key going up while key-up sounds are off picks nothing, so the up
/// lists' memory of what they played last is untouched. A random number is
/// drawn for the speed only when there is pitch variation, so with none the
/// choice of files is exactly what it would be without this.
pub fn voice(live: &Live, sounds: &SoundSet, kind: KeyKind, group: KeyGroup) -> Option<Sample> {
    if kind == KeyKind::Up && !live.key_up() {
        return None;
    }
    let sample = sounds.pick(kind, group)?;
    let variation = live.pitch_variation();
    let speed = if variation > 0.0 {
        speed_for(variation, sounds.random())
    } else {
        1.0
    };
    Some(sample.tuned(live.gain(), speed))
}
