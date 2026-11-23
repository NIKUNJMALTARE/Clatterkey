//! What the settings change while keys are being played: sound on or off,
//! the volume, the pitch variation, key-up sounds on or off, and the sound
//! set. The key thread reads it; the settings
//! side writes it. The key thread never waits for a writer.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use super::SoundSet;

/// The most the pitch variation can be.
pub const MAX_PITCH_VARIATION: f64 = 0.3;

/// The state shared by the player and the settings.
#[derive(Debug)]
pub struct Live {
    enabled: AtomicBool,
    /// The bits of the gain, an `f32`.
    gain: AtomicU32,
    /// The bits of the pitch variation, an `f32` from 0.0 to 0.3.
    pitch: AtomicU32,
    /// Whether a key going up plays its sound.
    key_up: AtomicBool,
    /// The sound set to play. Locked only to swap or copy the pointer.
    sounds: Mutex<Arc<SoundSet>>,
    /// Goes up each time `sounds` is replaced.
    version: AtomicU64,
}

impl Live {
    /// Sound on, at full volume, with no pitch variation and key-up sounds
    /// on, playing `sounds`.
    pub fn new(sounds: SoundSet) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            gain: AtomicU32::new(1.0_f32.to_bits()),
            pitch: AtomicU32::new(0.0_f32.to_bits()),
            key_up: AtomicBool::new(true),
            sounds: Mutex::new(Arc::new(sounds)),
            version: AtomicU64::new(0),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Sets the volume, 0.0 to 1.0. The gain is its square, so the slider's
    /// steps sound even; 1.0 leaves the samples as they are.
    pub fn set_volume(&self, volume: f64) {
        let volume = if volume.is_nan() {
            0.0
        } else {
            volume.clamp(0.0, 1.0)
        };
        let gain = (volume * volume) as f32;
        self.gain.store(gain.to_bits(), Ordering::Relaxed);
    }

    /// What each sample is multiplied by.
    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain.load(Ordering::Relaxed))
    }

    /// Sets how far each sound's speed may stray from normal, 0.0 to 0.3.
    /// At 0.0 every sound plays as recorded.
    pub fn set_pitch_variation(&self, variation: f64) {
        let variation = if variation.is_nan() {
            0.0
        } else {
            variation.clamp(0.0, MAX_PITCH_VARIATION)
        };
        self.pitch
            .store((variation as f32).to_bits(), Ordering::Relaxed);
    }

    pub fn pitch_variation(&self) -> f32 {
        f32::from_bits(self.pitch.load(Ordering::Relaxed))
    }

    /// Whether a key going up plays its sound, from the next key release.
    pub fn set_key_up(&self, on: bool) {
        self.key_up.store(on, Ordering::Relaxed);
    }

    pub fn key_up(&self) -> bool {
        self.key_up.load(Ordering::Relaxed)
    }

    /// The sound set in force now, waiting for a writer's pointer swap to
    /// finish. For the preview; not for the key path.
    pub fn snapshot(&self) -> Arc<SoundSet> {
        self.sounds().1
    }

    /// Plays `sounds` from the next key event. The set must be complete
    /// before it is given: loading and decoding happen before this call.
    pub fn set_sounds(&self, sounds: SoundSet) {
        let sounds = Arc::new(sounds);
        let old = {
            let mut current = self.sounds.lock().unwrap_or_else(|p| p.into_inner());
            let old = std::mem::replace(&mut *current, sounds);
            // Bumped under the lock, so a reader that sees the new version
            // and then gets the lock always finds a set at least that new.
            self.version.fetch_add(1, Ordering::Release);
            old
        };
        // The old set's samples are freed here, outside the lock, unless
        // the key thread still holds it.
        drop(old);
    }

    /// The version of the sound set, for `refresh`.
    pub(super) fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// The current sound set and its version, or `None` if a writer is
    /// swapping it at this instant. Never waits.
    pub(super) fn try_sounds(&self) -> Option<(u64, Arc<SoundSet>)> {
        let current = match self.sounds.try_lock() {
            Ok(current) => current,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        // Read under the lock: writers bump it under the same lock.
        Some((self.version.load(Ordering::Acquire), Arc::clone(&current)))
    }

    /// As `try_sounds`, waiting for a writer's pointer swap to finish. Not
    /// for the key path.
    pub(super) fn sounds(&self) -> (u64, Arc<SoundSet>) {
        let current = self.sounds.lock().unwrap_or_else(|p| p.into_inner());
        (self.version.load(Ordering::Acquire), Arc::clone(&current))
    }
}
