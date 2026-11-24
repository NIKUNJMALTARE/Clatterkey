//! Launch at login: what is decided, with no Tauri in it, so it is tested
//! directly. `native` is the only code that touches the autostart plugin.
//!
//! The system keeps the registration that starts the app at login, and the
//! user can change it there. The settings file keeps what the user chose
//! in the window. `reconcile` puts the two in step at start-up.

pub mod native;

use std::ffi::{OsStr, OsString};

use crate::permission::PermissionState;
use crate::{ErrorOut, write_line};

/// The system's registration that starts the app when the user logs in.
pub trait LoginItem: Send + Sync {
    /// Makes the registration, or refreshes it to where the app now is.
    fn enable(&self) -> Result<(), String>;
    /// Removes it. One that is already gone is not an error.
    fn disable(&self) -> Result<(), String>;
    /// Whether one is in force: present, and not switched off in the
    /// system where the system lets that be read.
    fn is_enabled(&self) -> Result<bool, String>;
    /// `false` for a registration that is not real, so that start-up does
    /// not compare the setting against it.
    fn tracks_system(&self) -> bool {
        true
    }
}

/// A registration that does nothing and reports none: development builds,
/// which must never leave a build folder in the user's login items, and
/// the plain core.
pub struct Inert;

impl LoginItem for Inert {
    fn enable(&self) -> Result<(), String> {
        Ok(())
    }

    fn disable(&self) -> Result<(), String> {
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, String> {
        Ok(false)
    }

    fn tracks_system(&self) -> bool {
        false
    }
}

/// The `launch_at_login` to start with, given the one in the settings file.
///
/// A registration that is gone or switched off in the system turns the
/// setting off: the user's choice there wins. One that is in force is
/// refreshed, since the app may have moved; it is refreshed only then, so
/// that nothing the user switched off is switched back on. A setting that
/// is off removes a registration that is still there. Whatever fails, the
/// setting keeps its value and one `autostart:` line is written.
pub fn reconcile(setting: bool, item: &dyn LoginItem, err: &ErrorOut) -> bool {
    if !item.tracks_system() {
        return setting;
    }
    let in_force = match item.is_enabled() {
        Ok(in_force) => in_force,
        Err(error) => {
            write_line(
                err,
                &format!("autostart: could not read the login item: {error}"),
            );
            return setting;
        }
    };
    match (setting, in_force) {
        (true, true) => {
            if let Err(error) = item.enable() {
                write_line(
                    err,
                    &format!("autostart: could not refresh the login item: {error}"),
                );
            }
            true
        }
        (true, false) => false,
        (false, true) => {
            if let Err(error) = item.disable() {
                write_line(
                    err,
                    &format!("autostart: could not remove the login item: {error}"),
                );
            }
            false
        }
        (false, false) => false,
    }
}

/// The argument the registration passes, which marks a start made by it.
pub const HIDDEN_ARG: &str = "--hidden";

/// Whether these are the arguments of a start made by the login
/// registration. The first is the program's own path and is not looked at.
pub fn is_login_start<S: AsRef<OsStr>>(args: &[S]) -> bool {
    args.iter().skip(1).any(|arg| arg.as_ref() == HIDDEN_ARG)
}

/// Whether the settings window is shown as the app starts. A start by hand
/// always shows it. A login start stays hidden, unless the app would then
/// be silent with no explanation (the permission is missing) or out of
/// reach (there is no tray icon).
pub fn show_at_start(login_start: bool, permission: PermissionState, tray_present: bool) -> bool {
    let can_listen = matches!(
        permission,
        PermissionState::Granted | PermissionState::NotNeeded
    );
    !(login_start && tray_present && can_listen)
}

/// Whether a second launch, with these arguments, shows the running app's
/// settings window: always, unless that launch is itself a login start.
pub fn show_on_second_launch<S: AsRef<OsStr>>(args: &[S]) -> bool {
    !is_login_start(args)
}

/// The arguments for "Restart now": the user is at the window, so the new
/// process must not start hidden.
pub fn without_hidden(args: Vec<OsString>) -> Vec<OsString> {
    let mut kept = Vec::with_capacity(args.len());
    for (index, arg) in args.into_iter().enumerate() {
        if index == 0 || arg != HIDDEN_ARG {
            kept.push(arg);
        }
    }
    kept
}
