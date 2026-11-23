//! The commands the settings window calls. Each is a thin wrapper over
//! `Core`, which holds the logic and is tested without Tauri.
//!
//! They are `command(async)`: a plain command runs on the main thread, and
//! loading a pack there would freeze the window.

mod core;
mod start;

use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::keys::permission::SystemAccess;
use crate::permission::{self, Os, Permission};

pub use core::{AudioStatus, Core, CoreParts, Listener};
pub use start::{plain_core, publish_core, publish_core_after, start_core};

/// Holds the `Core` once start-up has made it. A command that arrives
/// earlier waits; the window shows its loading state meanwhile.
#[derive(Default)]
pub struct CoreGate {
    core: Mutex<Option<Arc<Core>>>,
    ready: Condvar,
}

impl CoreGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes `core` available and wakes every waiting command. The first
    /// one published stays.
    pub fn publish(&self, core: Arc<Core>) {
        let mut slot = self.core.lock().unwrap_or_else(|p| p.into_inner());
        if slot.is_none() {
            *slot = Some(core);
        }
        self.ready.notify_all();
    }

    /// The `Core`, waiting until it is published.
    pub fn wait(&self) -> Arc<Core> {
        let mut slot = self.core.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(core) = slot.as_ref() {
                return Arc::clone(core);
            }
            slot = self.ready.wait(slot).unwrap_or_else(|p| p.into_inner());
        }
    }
}

#[tauri::command(async)]
pub fn list_packs(gate: State<'_, Arc<CoreGate>>) -> Value {
    gate.wait().list_packs()
}

#[tauri::command(async)]
pub fn get_settings(gate: State<'_, Arc<CoreGate>>) -> Value {
    gate.wait().get_settings()
}

#[tauri::command(async)]
pub fn set_settings(gate: State<'_, Arc<CoreGate>>, patch: Value) -> Result<Value, String> {
    gate.wait().set_settings(&patch)
}

#[tauri::command(async)]
pub fn audio_status(gate: State<'_, Arc<CoreGate>>) -> &'static str {
    gate.wait().audio_status()
}

#[tauri::command(async)]
pub fn permission_status(gate: State<'_, Arc<CoreGate>>) -> &'static str {
    gate.wait().permission_status()
}

/// The "Open System Settings" button. `false` where there is no permission
/// to give. It does not wait at the gate: it needs nothing from the `Core`.
#[tauri::command(async)]
pub fn open_permission_settings(permission: State<'_, Arc<Permission>>) -> Result<bool, String> {
    permission::open_settings(&SystemAccess, &permission)
}

/// The "Restart now" button: quits and starts the app again. `false`, and
/// nothing happens, where there is no permission to wait for.
#[tauri::command(async)]
pub fn restart_app(app: AppHandle) -> bool {
    if !SystemAccess.required() {
        return false;
    }
    app.restart()
}
