//! Turns key events into sounds.

use std::sync::{Arc, Mutex};

use super::{Live, Sample, SoundSet};
use crate::keys::{KeyEvent, group_of};

/// Somewhere a sound can be played. `play` returns at once and cannot fail:
/// with no output it does nothing. It is given the sound and nothing else,
/// so no key code can reach the audio layer. The sound carries its gain.
pub trait Sink: Send + Sync {
    fn play(&self, sample: &Sample);
}

/// Plays a sound for each key event. It runs on the key listener's thread,
/// so it only picks a prepared sound and hands it to the sink.
pub struct Player {
    sink: Arc<dyn Sink>,
    live: Arc<Live>,
    /// The sound set in use and its version. Only `handle` locks this, so
    /// nothing a settings change, a pack load or a file write holds can
    /// make a key event wait.
    current: Mutex<(u64, Arc<SoundSet>)>,
}

impl Player {
    /// Plays `click` for every key going down and nothing for keys going up.
    pub fn new(sink: Arc<dyn Sink>, click: Sample) -> Self {
        Self::with_sounds(sink, SoundSet::single(click))
    }

    /// Plays from `sounds`, by the key's group, always on and at full volume.
    pub fn with_sounds(sink: Arc<dyn Sink>, sounds: SoundSet) -> Self {
        Self::with_live(sink, Arc::new(Live::new(sounds)))
    }

    /// Plays as `live` says now: on or off, its volume, its sound set.
    pub fn with_live(sink: Arc<dyn Sink>, live: Arc<Live>) -> Self {
        let current = Mutex::new(live.sounds());
        Self {
            sink,
            live,
            current,
        }
    }

    /// Plays the sound for the key's group and direction, if there is one.
    /// The key is used only to find its group.
    pub fn handle(&self, event: KeyEvent) {
        if !self.live.enabled() {
            return;
        }
        let mut current = self.current.lock().unwrap_or_else(|p| p.into_inner());
        if current.0 != self.live.version() {
            // A new pack is ready. If it is being swapped in at this very
            // instant, this event plays the old one and the next looks again.
            if let Some(newer) = self.live.try_sounds() {
                *current = newer;
            }
        }
        if let Some(sample) = current.1.pick(event.kind, group_of(event.code)) {
            self.sink.play(&sample.with_gain(self.live.gain()));
        }
    }
}
