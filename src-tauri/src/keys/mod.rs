//! Global key listener: reports every key press and release system-wide,
//! with auto-repeat removed.
//!
//! This is the only module that talks to the OS key APIs (a native event tap
//! on macOS, `rdev` on Windows). No OS or library type leaves it, so a back
//! end can be replaced without touching its callers.

mod code;
mod filter;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::fmt;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::mpsc;
use std::thread::JoinHandle;

pub use code::KeyCode;
pub use filter::{KeyEvent, KeyKind, RawKey, RepeatFilter};

/// Why the listener could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyListenError {
    /// Key capture is compiled out on this platform.
    Unsupported,
    /// The OS refused to start the listener.
    Start(String),
    /// The listener started and then stopped unexpectedly.
    Stopped(String),
}

impl fmt::Display for KeyListenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyListenError::Unsupported => {
                f.write_str("key capture is not supported on this platform")
            }
            KeyListenError::Start(reason) => write!(f, "key listener could not start: {reason}"),
            KeyListenError::Stopped(reason) => write!(f, "key listener stopped: {reason}"),
        }
    }
}

impl std::error::Error for KeyListenError {}

/// A running listener. It runs for the life of the process; there is no way
/// to stop it.
#[derive(Debug)]
pub struct Listener {
    thread: JoinHandle<Result<(), KeyListenError>>,
}

impl Listener {
    /// Blocks until the listener thread ends, which normally it never does.
    pub fn wait(self) -> Result<(), KeyListenError> {
        self.thread.join().unwrap_or_else(|_| {
            Err(KeyListenError::Stopped(
                "the listener thread panicked".into(),
            ))
        })
    }
}

/// How long `start` waits on Windows for the hook to be refused before it
/// takes the listener to be running. A refusal comes back within a few
/// milliseconds; a failure after this is still reported by `Listener::wait`.
#[cfg(target_os = "windows")]
const WINDOWS_START_GRACE: std::time::Duration = std::time::Duration::from_millis(200);

/// How a back end tells `start` whether the OS accepted the listener.
#[cfg(any(target_os = "macos", target_os = "windows"))]
type Started = mpsc::Sender<Result<(), KeyListenError>>;

/// Starts listening on a dedicated thread and calls `consumer` on that thread
/// for every key event, whatever app has focus.
///
/// On macOS the process needs the Accessibility / Input Monitoring
/// permission; without it the OS may deliver nothing and report no error.
/// On Windows a second call replaces the first listener's consumer, and a
/// consumer that panics stops receiving events without ending the listener.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn start(
    mut consumer: impl FnMut(KeyEvent) + Send + 'static,
) -> Result<Listener, KeyListenError> {
    let (started, has_started) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("keys".into())
        .spawn(move || {
            let mut filter = RepeatFilter::new();
            let deliver = move |raw: RawKey| {
                if let Some(event) = filter.accept(raw) {
                    consumer(event);
                }
            };
            #[cfg(target_os = "macos")]
            return macos::run(deliver, started);
            #[cfg(target_os = "windows")]
            return windows::run(deliver, started);
        })
        .map_err(|error| KeyListenError::Start(error.to_string()))?;

    // macOS reports as soon as the tap exists or is refused. Windows can
    // only report a failure, so a short silence there means the hook is in.
    #[cfg(target_os = "macos")]
    let report = has_started.recv().ok();
    #[cfg(target_os = "windows")]
    let report = match has_started.recv_timeout(WINDOWS_START_GRACE) {
        Err(mpsc::RecvTimeoutError::Timeout) => Some(Ok(())),
        other => other.ok(),
    };

    match report {
        Some(Ok(())) => Ok(Listener { thread }),
        Some(Err(error)) => Err(error),
        // The thread ended without reporting: pass on whatever it returned.
        None => match (Listener { thread }).wait() {
            Err(error) => Err(error),
            Ok(()) => Err(KeyListenError::Start("the listener ended at once".into())),
        },
    }
}

/// Key capture is compiled out on this platform (Linux is used only to run
/// the checks), so this always fails.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn start(_consumer: impl FnMut(KeyEvent) + Send + 'static) -> Result<Listener, KeyListenError> {
    Err(KeyListenError::Unsupported)
}
