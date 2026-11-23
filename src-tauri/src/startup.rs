//! Connects the key listener to the click. The listener is passed in, so the
//! failure paths are tested without real key capture.

use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use crate::audio::{Player, Sink, builtin_click};
use crate::keys::{KeyEvent, KeyListenError};

/// Receives key events on the listener's thread.
pub type Consumer = Box<dyn FnMut(KeyEvent) + Send>;

/// Blocks until the listener ends.
pub type Wait = Box<dyn FnOnce() -> Result<(), KeyListenError>>;

/// Where errors are reported: standard error in the app.
pub type ErrorOut = Arc<Mutex<dyn Write + Send>>;

/// The consumer to give the key listener: it plays a click for each key that
/// goes down, and writes nothing per key.
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
                "click playback failed; there is no sound until the app is restarted",
            );
        }
    }
}

/// Prepares the click, starts the listener with a consumer that plays it
/// into `sink`, and waits for the listener to end, which normally it never
/// does. If the listener cannot start, or stops, the error is written to
/// `err` once and this returns; the app carries on without clicks.
pub fn run_clicks(
    start: impl FnOnce(Consumer) -> Result<Wait, KeyListenError>,
    sink: Arc<dyn Sink>,
    err: ErrorOut,
) {
    let player = Player::new(sink, builtin_click());
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
