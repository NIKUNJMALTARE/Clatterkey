//! Dev command: prints every key press and release, system-wide, until
//! stopped with Ctrl+C. Run it with `npm run keys`.
//!
//! This is a development tool. It is an example target, so none of it is
//! compiled into the app, which never prints or logs keys.

use std::io::Write;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use clickclak_lib::keys::{self, KeyEvent, KeyListenError};

#[cfg(test)]
mod tests;

/// Blocks until the listener ends.
type Wait = Box<dyn FnOnce() -> Result<(), KeyListenError>>;

#[cfg(target_os = "macos")]
const START_LINE: &str = "Listening for keys; press Ctrl+C to stop. On macOS the terminal running \
    this needs the Input Monitoring permission (System Settings, Privacy & Security); without \
    it nothing is printed.";
#[cfg(not(target_os = "macos"))]
const START_LINE: &str = "Listening for keys; press Ctrl+C to stop.";

/// One output line for an event, such as `down KeyA`.
fn format_line(event: KeyEvent) -> String {
    format!("{} {}", event.kind, event.code)
}

/// Prints the start-up line, starts the listener with a consumer that prints
/// one line per event, and waits for it to end. `start` is a parameter so the
/// failure paths can be tested without a real listener.
fn run(
    start: impl FnOnce(Box<dyn FnMut(KeyEvent) + Send>) -> Result<Wait, KeyListenError>,
    out: Arc<Mutex<dyn Write + Send>>,
    err: &mut dyn Write,
) -> ExitCode {
    print_line(&out, START_LINE);

    let events_out = Arc::clone(&out);
    let consumer = Box::new(move |event| print_line(&events_out, &format_line(event)));

    match start(consumer).and_then(|wait| wait()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(err, "error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Writes one line and flushes it, so lines appear as keys are pressed.
/// Write errors (a closed pipe, say) are ignored: there is nowhere to report
/// them.
fn print_line(out: &Mutex<dyn Write + Send>, line: &str) {
    let mut out = out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn main() -> ExitCode {
    run(
        |consumer| {
            let listener = keys::start(consumer)?;
            Ok(Box::new(move || listener.wait()) as Wait)
        },
        Arc::new(Mutex::new(std::io::stdout())),
        &mut std::io::stderr(),
    )
}
