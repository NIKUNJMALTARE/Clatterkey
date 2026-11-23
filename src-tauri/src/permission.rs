//! The macOS key-listening permission (Input Monitoring): what state it is
//! in, and when to start or stop the key listener because of it.
//!
//! Plain Rust. The OS and the listener are reached through two small traits,
//! so every decision here is tested with fakes on any platform. The real OS
//! calls are in `keys::permission`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use crate::keys::KeyListenError;
use crate::{ErrorOut, write_line};

/// How often the watcher checks while the listener is not running.
pub const CHECK_WHILE_MISSING: Duration = Duration::from_secs(1);

/// How often it checks once the listener runs, to notice the permission
/// being taken away.
pub const CHECK_WHILE_GRANTED: Duration = Duration::from_secs(5);

/// What the app knows about the permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// This platform asks for no permission.
    NotNeeded,
    /// macOS says the app may not listen to keys.
    Needed,
    /// macOS says it may, and the key listener is running.
    Granted,
    /// macOS says it may, but the listener could not be started in this run.
    RestartNeeded,
}

impl PermissionState {
    /// The name the settings window reads.
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionState::NotNeeded => "not_needed",
            PermissionState::Needed => "needed",
            PermissionState::Granted => "granted",
            PermissionState::RestartNeeded => "restart_needed",
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            0 => PermissionState::NotNeeded,
            2 => PermissionState::Granted,
            3 => PermissionState::RestartNeeded,
            _ => PermissionState::Needed,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            PermissionState::NotNeeded => 0,
            PermissionState::Needed => 1,
            PermissionState::Granted => 2,
            PermissionState::RestartNeeded => 3,
        }
    }
}

/// The permission state, shared between the watcher that sets it and the
/// commands that report it. Reading it takes no lock and never waits.
#[derive(Debug)]
pub struct Permission {
    state: AtomicU8,
    asked: AtomicBool,
}

impl Permission {
    pub fn new(initial: PermissionState) -> Self {
        Self {
            state: AtomicU8::new(initial.to_u8()),
            asked: AtomicBool::new(false),
        }
    }

    pub fn state(&self) -> PermissionState {
        PermissionState::from_u8(self.state.load(Ordering::SeqCst))
    }

    pub fn set(&self, state: PermissionState) {
        self.state.store(state.to_u8(), Ordering::SeqCst);
    }

    /// Whether the user has asked macOS for the permission in this run.
    pub fn asked(&self) -> bool {
        self.asked.load(Ordering::SeqCst)
    }

    pub fn mark_asked(&self) {
        self.asked.store(true, Ordering::SeqCst);
    }
}

/// The operating system's side of the permission.
pub trait Os: Send + Sync {
    /// Whether this platform has such a permission at all (only macOS).
    fn required(&self) -> bool;
    /// Whether the OS reports that the app may listen to keys. Shows nothing.
    fn granted(&self) -> bool;
    /// Asks the OS for the permission, which lists the app in its settings
    /// and may show the OS's own prompt.
    fn request(&self);
    /// Opens the OS settings on the pane where the permission is given.
    fn open_pane(&self) -> Result<(), String>;
}

/// The key listener, as the watcher drives it.
pub trait Keys {
    fn start(&mut self) -> Result<(), KeyListenError>;
    fn stop(&mut self);
    /// False once the listener has been stopped or has ended by itself.
    fn running(&self) -> bool;
    /// Whether the listener running now has delivered a key event. That is
    /// proof the OS lets it listen, whatever the OS says when asked. A
    /// listener that cannot tell says yes.
    fn heard(&self) -> bool {
        true
    }
}

/// The state before the first check. Asks the OS for nothing.
pub fn initial_state(os: &dyn Os) -> PermissionState {
    if os.required() {
        PermissionState::Needed
    } else {
        PermissionState::NotNeeded
    }
}

/// The "Open System Settings" button: asks the OS for the permission and
/// opens its pane. `Ok(false)` where there is no such permission, and then
/// nothing is done. The state is left to the watcher.
pub fn open_settings(os: &dyn Os, permission: &Permission) -> Result<bool, String> {
    if !os.required() {
        return Ok(false);
    }
    os.request();
    permission.mark_asked();
    os.open_pane()?;
    Ok(true)
}

/// Keeps the listener and the reported state in step with the permission.
pub struct Watcher<O, K> {
    os: Arc<O>,
    keys: K,
    permission: Arc<Permission>,
    err: ErrorOut,
    /// Whether this watcher started the listener and has not stopped it.
    started: bool,
    /// The listener was started while the OS still said no (see `step`), so
    /// the OS's answer is not a reason to stop it, and the state is
    /// `Granted` only once a key has been heard through it.
    probed: bool,
    /// A failed start has been written to `err` and not yet followed by a
    /// start that worked.
    failure_reported: bool,
}

impl<O: Os, K: Keys> Watcher<O, K> {
    pub fn new(os: Arc<O>, keys: K, permission: Arc<Permission>, err: ErrorOut) -> Self {
        Self {
            os,
            keys,
            permission,
            err,
            started: false,
            probed: false,
            failure_reported: false,
        }
    }

    /// One check. Returns how long to wait before the next.
    pub fn step(&mut self) -> Duration {
        if self.started && !self.keys.running() {
            // The listener ended by itself. Clear it away; it is started
            // afresh below if the permission still allows.
            self.keys.stop();
            self.started = false;
            self.probed = false;
            write_line(&self.err, "key listener: stopped by itself");
        }

        let state = if self.os.granted() {
            self.probed = false;
            if self.started {
                PermissionState::Granted
            } else {
                self.start_reporting()
            }
        } else if self.started && !self.probed {
            // The permission was taken away.
            self.keys.stop();
            self.started = false;
            PermissionState::Needed
        } else {
            // The OS's answer can be stale inside a running process. Once
            // the user has pressed the button, a listener is started
            // quietly as a second way to see the grant. Being able to start
            // it is not proof, since an OS may let it start and deliver
            // nothing; a key heard through it is.
            if !self.started && self.permission.asked() && self.keys.start().is_ok() {
                self.started = true;
                self.probed = true;
                self.failure_reported = false;
            }
            if self.started && self.keys.heard() {
                PermissionState::Granted
            } else {
                PermissionState::Needed
            }
        };

        self.permission.set(state);
        if state == PermissionState::Granted {
            CHECK_WHILE_GRANTED
        } else {
            CHECK_WHILE_MISSING
        }
    }

    /// Starts the listener with the permission reported. A failure is
    /// written once, however many checks it lasts.
    fn start_reporting(&mut self) -> PermissionState {
        match self.keys.start() {
            Ok(()) => {
                self.started = true;
                self.failure_reported = false;
                PermissionState::Granted
            }
            Err(error) => {
                if !self.failure_reported {
                    self.failure_reported = true;
                    let reason = error.to_string();
                    // The listener's own messages begin "key listener …".
                    let reason = reason.strip_prefix("key listener ").unwrap_or(&reason);
                    write_line(&self.err, &format!("key listener: {reason}"));
                }
                PermissionState::RestartNeeded
            }
        }
    }

    /// Checks for the life of the process, sleeping between checks.
    pub fn watch(mut self, mut sleep: impl FnMut(Duration)) {
        loop {
            let wait = self.step();
            sleep(wait);
        }
    }
}
