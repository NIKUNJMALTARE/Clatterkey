//! Connects the key listener to the sounds: chooses the sound pack from
//! the settings, then starts the listener. The listener is passed in, so the failure paths are
//! tested without real key capture.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::audio::{Live, Player, Sink, SoundSet, builtin_click};
use crate::keys::{KeyEvent, KeyListenError};
use crate::packs;
use crate::permission::Keys;
use crate::settings::DEFAULT_PACK;

/// Receives key events on the listener's thread.
pub type Consumer = Box<dyn FnMut(KeyEvent) + Send>;

/// Blocks until the listener ends.
pub type Wait = Box<dyn FnOnce() -> Result<(), KeyListenError>>;

pub use crate::{ErrorOut, write_line};

/// A listener that has been started: how to stop it, and whether it has
/// ended by itself.
pub struct Running {
    pub stop: Box<dyn FnOnce() + Send>,
    pub finished: Box<dyn Fn() -> bool + Send>,
}

/// The key listener as the permission watcher drives it: it can be started
/// more than once, one at a time. Every start plays as `live` says at that
/// moment, into `sink`.
pub struct LiveKeys<S> {
    start: S,
    sink: Arc<dyn Sink>,
    live: Arc<Live>,
    err: ErrorOut,
    running: Option<Running>,
    /// Set by the listener running now when its first key event arrives.
    heard: Arc<AtomicBool>,
}

impl<S> LiveKeys<S>
where
    S: FnMut(Consumer) -> Result<Running, KeyListenError>,
{
    pub fn new(start: S, sink: Arc<dyn Sink>, live: Arc<Live>, err: ErrorOut) -> Self {
        Self {
            start,
            sink,
            live,
            err,
            running: None,
            heard: Arc::default(),
        }
    }
}

impl<S> Keys for LiveKeys<S>
where
    S: FnMut(Consumer) -> Result<Running, KeyListenError>,
{
    fn start(&mut self) -> Result<(), KeyListenError> {
        // Never two listeners: the one before is ended first.
        self.stop();
        let player = Player::with_live(Arc::clone(&self.sink), Arc::clone(&self.live));
        let mut play = key_consumer(player, Arc::clone(&self.err));
        // Each listener has its own flag, so a key heard by the one before
        // says nothing about this one. On the key path this is one atomic
        // read, and one write for the first key only.
        let heard = Arc::new(AtomicBool::new(false));
        self.heard = Arc::clone(&heard);
        let consumer = Box::new(move |event| {
            if !heard.load(Ordering::Relaxed) {
                heard.store(true, Ordering::Relaxed);
            }
            play(event);
        });
        self.running = Some((self.start)(consumer)?);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            (running.stop)();
        }
    }

    fn running(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| !(running.finished)())
    }

    fn heard(&self) -> bool {
        self.running.is_some() && self.heard.load(Ordering::Relaxed)
    }
}

/// The consumer to give the key listener: it plays the sound for each key
/// event, and writes nothing per key.
///
/// It never panics, because a panic on the listener's thread would end key
/// capture, or the process. If playing a click panics, that is reported once
/// and no more clicks are played.
pub fn key_consumer(player: Player, err: ErrorOut) -> impl FnMut(KeyEvent) + Send + 'static {
    let mut failed = false;
    move |event| {
        if failed {
            return;
        }
        if catch_unwind(AssertUnwindSafe(|| player.handle(event))).is_err() {
            failed = true;
            write_line(
                &err,
                "sound playback failed; there is no sound until the app is restarted",
            );
        }
    }
}

/// Whether `id` can name a bundled pack: non-empty, and only ASCII letters,
/// digits, `-` and `_`. This rules out path separators, `.`, `..`, `:` and
/// drive prefixes on every OS before any path is built from it.
pub fn is_pack_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The sounds to play: the bundled pack `pack_id` in the `packs` folder,
/// else the default pack, else the built-in click. Each fallback writes one
/// `sound pack:` line to `err` saying why, so at most two lines are written.
/// `packs` is `None` when the bundled packs folder could not be found.
pub fn choose_sounds(packs: Option<&Path>, pack_id: &str, err: &ErrorOut) -> SoundSet {
    choose_pack(packs, pack_id, err).sounds
}

/// The sounds chosen at start-up and how the choice went.
#[derive(Debug)]
pub struct Chosen {
    pub sounds: SoundSet,
    /// The id of the pack that is playing; `None` for the built-in click.
    pub playing: Option<String>,
    /// Whether the pack asked for could not be used.
    pub fell_back: bool,
}

/// As `choose_sounds`, also saying which pack is playing and whether that
/// is a fallback, for the settings window.
pub fn choose_pack(packs: Option<&Path>, pack_id: &str, err: &ErrorOut) -> Chosen {
    let chosen = |sounds, playing: &str, fell_back| Chosen {
        sounds,
        playing: Some(playing.to_string()),
        fell_back,
    };
    let fallback = format!("using {DEFAULT_PACK:?}");
    if pack_id != DEFAULT_PACK {
        match packs {
            Some(dir) if is_pack_id(pack_id) && names_a_pack(dir, pack_id) => {
                match load_pack(&dir.join(pack_id)) {
                    Ok(sounds) => return chosen(sounds, pack_id, false),
                    Err(reason) => write_line(err, &format!("sound pack: {reason}; {fallback}")),
                }
            }
            Some(_) => write_line(
                err,
                &format!("sound pack: unknown pack {pack_id:?}; {fallback}"),
            ),
            // With no packs folder the default pack fails too; that line
            // says why.
            None => {}
        }
    }
    let reason = match packs {
        Some(dir) => match load_pack(&dir.join(DEFAULT_PACK)) {
            Ok(sounds) => return chosen(sounds, DEFAULT_PACK, pack_id != DEFAULT_PACK),
            Err(reason) => reason,
        },
        None => "the bundled packs folder was not found".to_string(),
    };
    write_line(
        err,
        &format!("sound pack: {reason}; using the built-in click"),
    );
    Chosen {
        sounds: SoundSet::single(builtin_click()),
        playing: None,
        fell_back: true,
    }
}

/// Whether `id` is exactly the name of a folder in `packs`. Matching on
/// the listed names, not by opening the path, keeps ids case-sensitive on
/// macOS and Windows too, so `"Brown"` is unknown everywhere.
pub(crate) fn names_a_pack(packs: &Path, id: &str) -> bool {
    std::fs::read_dir(packs).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| e.file_name() == id && e.path().is_dir())
    })
}

/// Loads a pack folder. A decoder that panics on a malformed file must not
/// take the app's sound with it: that is treated as an invalid pack.
pub(crate) fn load_pack(dir: &Path) -> Result<SoundSet, String> {
    match catch_unwind(AssertUnwindSafe(|| packs::load(dir))) {
        Ok(Ok(pack)) => Ok(pack.sounds),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!(
            "pack {:?}: could not be decoded",
            dir.display().to_string()
        )),
    }
}

/// Plays the built-in click for every key that goes down. See `run_sounds`.
pub fn run_clicks(
    start: impl FnOnce(Consumer) -> Result<Wait, KeyListenError>,
    sink: Arc<dyn Sink>,
    err: ErrorOut,
) {
    run_sounds(start, sink, SoundSet::single(builtin_click()), err);
}

/// Starts the listener with a consumer that plays `sounds` into `sink`, and
/// waits for the listener to end, which normally it never does. If the
/// listener cannot start, or stops, the error is written to `err` once and
/// this returns; the app carries on without sound.
pub fn run_sounds(
    start: impl FnOnce(Consumer) -> Result<Wait, KeyListenError>,
    sink: Arc<dyn Sink>,
    sounds: SoundSet,
    err: ErrorOut,
) {
    run_live(start, sink, Arc::new(Live::new(sounds)), err);
}

/// As `run_sounds`, playing as `live` says: the settings can turn the sound
/// off, change the volume and swap the sound set while the listener runs.
pub fn run_live(
    start: impl FnOnce(Consumer) -> Result<Wait, KeyListenError>,
    sink: Arc<dyn Sink>,
    live: Arc<Live>,
    err: ErrorOut,
) {
    let player = Player::with_live(sink, live);
    let consumer = Box::new(key_consumer(player, Arc::clone(&err)));

    if let Err(error) = start(consumer).and_then(|wait| wait()) {
        let reason = error.to_string();
        // The listener's own messages begin "key listener …"; say it once.
        let reason = reason.strip_prefix("key listener ").unwrap_or(&reason);
        write_line(&err, &format!("key listener: {reason}"));
    }
}
