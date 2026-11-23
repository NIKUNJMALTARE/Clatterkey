//! Turns key events into sounds.

use std::sync::Arc;

use super::{Sample, SoundSet};
use crate::keys::{KeyEvent, group_of};

/// Somewhere a sound can be played. `play` returns at once and cannot fail:
/// with no output it does nothing. It is given the sound and nothing else,
/// so no key code can reach the audio layer.
pub trait Sink: Send + Sync {
    fn play(&self, sample: &Sample);
}

/// Plays a sound for each key event. It runs on the key listener's thread,
/// so it only picks a prepared sound and hands it to the sink.
pub struct Player {
    sink: Arc<dyn Sink>,
    sounds: SoundSet,
}

impl Player {
    /// Plays `click` for every key going down and nothing for keys going up.
    pub fn new(sink: Arc<dyn Sink>, click: Sample) -> Self {
        Self::with_sounds(sink, SoundSet::single(click))
    }

    /// Plays from `sounds`, by the key's group.
    pub fn with_sounds(sink: Arc<dyn Sink>, sounds: SoundSet) -> Self {
        Self { sink, sounds }
    }

    /// Plays the sound for the key's group and direction, if there is one.
    /// The key is used only to find its group.
    pub fn handle(&self, event: KeyEvent) {
        if let Some(sample) = self.sounds.pick(event.kind, group_of(event.code)) {
            self.sink.play(sample);
        }
    }
}
