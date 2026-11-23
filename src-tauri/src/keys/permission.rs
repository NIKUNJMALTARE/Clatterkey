//! The operating system's side of the key-listening permission. On macOS
//! that is Input Monitoring, which gates the listen-only event tap; nowhere
//! else is there such a permission. The decisions made from these answers
//! are in `crate::permission`.

use crate::permission::Os;

/// The real operating system.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemAccess;

#[cfg(target_os = "macos")]
mod imp {
    use std::process::{Command, Stdio};

    use objc2_core_graphics::{CGPreflightListenEventAccess, CGRequestListenEventAccess};

    /// System Settings, Privacy & Security, Input Monitoring.
    const INPUT_MONITORING_PANE: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

    pub const REQUIRED: bool = true;

    pub fn granted() -> bool {
        CGPreflightListenEventAccess()
    }

    pub fn request() {
        // The answer is the state now, which the watcher reads for itself.
        let _ = CGRequestListenEventAccess();
    }

    pub fn open_pane() -> Result<(), String> {
        let status = Command::new("/usr/bin/open")
            .arg(INPUT_MONITORING_PANE)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("open ended with {status}"))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub const REQUIRED: bool = false;

    pub fn granted() -> bool {
        true
    }

    pub fn request() {}

    pub fn open_pane() -> Result<(), String> {
        Ok(())
    }
}

impl Os for SystemAccess {
    fn required(&self) -> bool {
        imp::REQUIRED
    }

    fn granted(&self) -> bool {
        imp::granted()
    }

    fn request(&self) {
        imp::request();
    }

    fn open_pane(&self) -> Result<(), String> {
        imp::open_pane()
    }
}
