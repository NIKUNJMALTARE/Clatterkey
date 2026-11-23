//! The Preview button: a short typed phrase played through the same sink
//! as key presses, with the sounds and settings in force. It is built from
//! key groups, never from key codes, and runs on its own thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::{Live, Sink, SoundSet, voice};
use crate::keys::{KeyGroup, KeyKind};

/// The phrase: four letters, Space, two letters, Enter.
pub const PHRASE: [KeyGroup; 8] = [
    KeyGroup::Default,
    KeyGroup::Default,
    KeyGroup::Default,
    KeyGroup::Default,
    KeyGroup::Space,
    KeyGroup::Default,
    KeyGroup::Default,
    KeyGroup::Enter,
];

/// From a key going down to it coming up.
pub const HOLD: Duration = Duration::from_millis(60);

/// From a key going down to the next key going down.
pub const GAP: Duration = Duration::from_millis(110);

/// Plays the phrase from `sounds`: each key's down sound, then after `HOLD`
/// its up sound if key-up sounds are on and the set has one. The gain, the
/// pitch variation and the key-up setting are read from `live` as each
/// sound is played. Whether sound is on is not looked at: a preview was
/// asked for. It stops, playing nothing more, once `cancelled` says so.
pub fn play_phrase(
    sink: &dyn Sink,
    live: &Live,
    sounds: &SoundSet,
    cancelled: impl Fn() -> bool,
    mut sleep: impl FnMut(Duration),
) {
    for group in PHRASE {
        if cancelled() {
            return;
        }
        if let Some(sample) = voice(live, sounds, KeyKind::Down, group) {
            sink.play(&sample);
        }
        sleep(HOLD);
        if cancelled() {
            return;
        }
        if let Some(sample) = voice(live, sounds, KeyKind::Up, group) {
            sink.play(&sample);
        }
        sleep(GAP - HOLD);
    }
}

/// Starts previews, one at a time: a new one ends the one before it.
pub struct Previewer {
    sink: Arc<dyn Sink>,
    /// Goes up with every preview started. A preview stops when it is no
    /// longer the newest.
    generation: Arc<AtomicU64>,
}

impl Previewer {
    /// Previews play into `sink`, the one key presses play into.
    pub fn new(sink: Arc<dyn Sink>) -> Self {
        Self {
            sink,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Starts the phrase with the sound set `live` has now and returns at
    /// once. A preview already playing plays no further keys. The error is
    /// why the preview's thread could not be started.
    pub fn start(&self, live: Arc<Live>) -> Result<(), String> {
        let mine = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let generation = Arc::clone(&self.generation);
        let sink = Arc::clone(&self.sink);
        // Taken here, so a pack chosen after the press does not change it.
        let sounds = live.snapshot();
        std::thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                play_phrase(
                    sink.as_ref(),
                    &live,
                    &sounds,
                    || generation.load(Ordering::SeqCst) != mine,
                    std::thread::sleep,
                );
            })
            .map(|_| ())
            .map_err(|error| format!("the preview could not be started: {error}"))
    }
}
