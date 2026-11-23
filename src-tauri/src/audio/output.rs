//! Keeps a sound output open: opens the default device, notices when it
//! fails or the default changes, and opens the new one. No `rodio` here; the
//! real device is behind `Backend`, so this is tested with a fake.

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;

use super::device::DeviceBackend;
use super::{Sample, Sink};

/// How often the output is checked when nothing has reported a failure. It
/// bounds how long clicks stay silent after a device appears or changes.
const CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// How long to wait after a stream reports a failure before re-opening.
const REOPEN_DELAY: Duration = Duration::from_millis(500);

/// Why an output could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// There is no output device to open.
    NoDevice,
    /// There is a device and it could not be opened.
    Open(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::NoDevice => f.write_str("no audio output device"),
            AudioError::Open(reason) => write!(f, "audio output could not be opened: {reason}"),
        }
    }
}

impl std::error::Error for AudioError {}

/// What the output is doing now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStatus {
    /// An output is open and clicks are heard.
    Playing,
    /// There is no output device; clicks are silent until one appears.
    NoDevice,
    /// A device exists and could not be opened; it is retried.
    Failed,
}

impl OutputStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            0 => OutputStatus::Playing,
            1 => OutputStatus::NoDevice,
            _ => OutputStatus::Failed,
        }
    }
}

/// Reads the output's status from any thread.
#[derive(Debug, Clone)]
pub struct StatusHandle(Arc<AtomicU8>);

impl StatusHandle {
    fn new(status: OutputStatus) -> Self {
        Self(Arc::new(AtomicU8::new(status as u8)))
    }

    fn set(&self, status: OutputStatus) {
        self.0.store(status as u8, Ordering::Relaxed);
    }

    pub fn get(&self) -> OutputStatus {
        OutputStatus::from_u8(self.0.load(Ordering::Relaxed))
    }
}

/// The sink the key path plays into. It forwards to whichever output is open
/// now and drops the sound when there is none: nothing is queued.
#[derive(Default)]
pub struct SinkSlot {
    current: Mutex<Option<Arc<dyn Sink>>>,
}

impl SinkSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes `sink` the output from now on.
    pub fn set(&self, sink: Arc<dyn Sink>) {
        *self.lock() = Some(sink);
    }

    /// Leaves no output; sounds are dropped until the next `set`.
    pub fn clear(&self) {
        *self.lock() = None;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Arc<dyn Sink>>> {
        // The guarded value is a plain pointer swap, so a poisoned lock is
        // still in a usable state.
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Sink for SinkSlot {
    /// Never waits. If the output is being swapped at this instant the sound
    /// is dropped, as it would be with no output. The lock is released
    /// before the sound is handed on.
    fn play(&self, sample: &Sample) {
        let sink = match self.current.try_lock() {
            Ok(current) => current.clone(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().clone(),
            Err(TryLockError::WouldBlock) => None,
        };
        if let Some(sink) = sink {
            sink.play(sample);
        }
    }
}

/// A sound device, as the supervisor sees it.
pub trait Backend {
    /// The open output. Dropping it closes the output.
    type Stream;

    /// Opens the current default output and returns it with the sink that
    /// plays into it.
    fn open(&mut self) -> Result<(Self::Stream, Arc<dyn Sink>), AudioError>;

    /// Whether `stream` should be replaced: it has failed, or the system's
    /// default output is no longer the device it was opened on.
    fn is_stale(&mut self, stream: &Self::Stream) -> bool;
}

/// Keeps `slot` pointing at an open output. Call `tick` regularly.
pub struct Supervisor<B: Backend> {
    backend: B,
    slot: Arc<SinkSlot>,
    stream: Option<B::Stream>,
    status: StatusHandle,
}

impl<B: Backend> Supervisor<B> {
    pub fn new(backend: B, slot: Arc<SinkSlot>) -> Self {
        Self {
            backend,
            slot,
            stream: None,
            status: StatusHandle::new(OutputStatus::NoDevice),
        }
    }

    /// A handle that keeps reporting the status as it changes.
    pub fn status_handle(&self) -> StatusHandle {
        self.status.clone()
    }

    /// One check: closes an output that has gone stale, and opens one if
    /// none is open. A failure to open is retried on the next call.
    pub fn tick(&mut self) -> OutputStatus {
        if let Some(stream) = &self.stream
            && self.backend.is_stale(stream)
        {
            self.slot.clear();
            self.stream = None;
        }

        let status = if self.stream.is_some() {
            OutputStatus::Playing
        } else {
            match self.backend.open() {
                Ok((stream, sink)) => {
                    self.stream = Some(stream);
                    self.slot.set(sink);
                    OutputStatus::Playing
                }
                Err(AudioError::NoDevice) => OutputStatus::NoDevice,
                Err(AudioError::Open(_)) => OutputStatus::Failed,
            }
        };
        self.status.set(status);
        status
    }
}

/// The running output. It lasts for the life of the process.
#[derive(Debug, Clone)]
pub struct OutputHandle {
    status: StatusHandle,
}

impl OutputHandle {
    pub fn status(&self) -> OutputStatus {
        self.status.get()
    }
}

/// Starts the `audio` thread, which opens the default output device, keeps
/// `slot` playing into it, and follows device changes. The output stream is
/// created and kept on that thread. If the thread cannot be started the app
/// carries on without sound.
pub fn spawn_output(slot: Arc<SinkSlot>) -> OutputHandle {
    let (wake, woken) = mpsc::channel();
    let mut supervisor = Supervisor::new(DeviceBackend::new(wake), slot);
    let status = supervisor.status_handle();

    let spawned = std::thread::Builder::new()
        .name("audio".into())
        .spawn(move || {
            loop {
                supervisor.tick();
                // Wait for the stream to report a failure, or for the next
                // routine check.
                match woken.recv_timeout(CHECK_INTERVAL) {
                    // A failure: give the device a moment before re-opening,
                    // so an output that fails as soon as it opens is retried
                    // at a steady pace and not in a tight loop.
                    Ok(()) => std::thread::sleep(REOPEN_DELAY),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => std::thread::sleep(CHECK_INTERVAL),
                }
                // One failure can be reported many times; one check answers
                // them all.
                while woken.try_recv().is_ok() {}
            }
        });
    if spawned.is_err() {
        status.set(OutputStatus::Failed);
    }
    OutputHandle { status }
}
