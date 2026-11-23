//! Turns key events into clicks.

use std::sync::Arc;

use super::Sample;
use crate::keys::{KeyEvent, KeyKind};

/// Somewhere a sound can be played. `play` returns at once and cannot fail:
/// with no output it does nothing. It is given the sound and nothing else,
/// so no key code can reach the audio layer.
pub trait Sink: Send + Sync {
    fn play(&self, sample: &Sample);
}

/// Plays the click for every key that goes down. It runs on the key
/// listener's thread, so it only hands the prepared sound to the sink.
pub struct Player {
    sink: Arc<dyn Sink>,
    click: Sample,
}

impl Player {
    pub fn new(sink: Arc<dyn Sink>, click: Sample) -> Self {
        Self { sink, click }
    }

    /// A `Down` plays the click; an `Up` plays nothing. Which key it was is
    /// not looked at.
    pub fn handle(&self, event: KeyEvent) {
        if event.kind == KeyKind::Down {
            self.sink.play(&self.click);
        }
    }
}
