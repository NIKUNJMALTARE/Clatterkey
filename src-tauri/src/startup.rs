//! Connects the key listener to the sounds: chooses the sound pack, then
//! starts the listener. The listener is passed in, so the failure paths are
//! tested without real key capture.

use std::ffi::OsString;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::audio::{Player, Sink, SoundSet, builtin_click};
use crate::keys::{KeyEvent, KeyListenError};
use crate::packs;

/// Receives key events on the listener's thread.
pub type Consumer = Box<dyn FnMut(KeyEvent) + Send>;

/// Blocks until the listener ends.
pub type Wait = Box<dyn FnOnce() -> Result<(), KeyListenError>>;

/// Where errors are reported: standard error in the app.
pub type ErrorOut = Arc<Mutex<dyn Write + Send>>;

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

/// The pack folder named by the `MKS_PACK` environment variable's value.
/// Unset and empty both mean no pack.
pub fn pack_dir(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// The sounds to play: the pack in `pack` if it loads, otherwise the
/// built-in click. With no pack nothing is written; with a pack that cannot
/// be loaded, one line saying why is written to `err`.
pub fn choose_sounds(pack: Option<&Path>, err: &ErrorOut) -> SoundSet {
    let builtin = || SoundSet::single(builtin_click());
    let Some(dir) = pack else {
        return builtin();
    };
    // A decoder that panics on a malformed file must not take the app's
    // sound with it: treat it as an invalid pack.
    let reason = match catch_unwind(AssertUnwindSafe(|| packs::load(dir))) {
        Ok(Ok(pack)) => return pack.sounds,
        Ok(Err(error)) => error.to_string(),
        Err(_) => format!("pack {:?}: could not be decoded", dir.display().to_string()),
    };
    write_line(
        err,
        &format!("sound pack: {reason}; using the built-in click"),
    );
    builtin()
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
    let player = Player::with_sounds(sink, sounds);
    let consumer = Box::new(key_consumer(player, Arc::clone(&err)));

    if let Err(error) = start(consumer).and_then(|wait| wait()) {
        let reason = error.to_string();
        // The listener's own messages begin "key listener …"; say it once.
        let reason = reason.strip_prefix("key listener ").unwrap_or(&reason);
        write_line(&err, &format!("key listener: {reason}"));
    }
}

/// Writes one line. Write errors are ignored: there is nowhere to report them.
fn write_line(out: &Mutex<dyn Write + Send>, line: &str) {
    let mut out = out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}
