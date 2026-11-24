//! The commands the settings window calls. Each is a thin wrapper over
//! `Core`, which holds the logic and is tested without Tauri.
//!
//! They are `command(async)`: a plain command runs on the main thread, and
//! loading a pack there would freeze the window.

mod core;
mod start;

use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::audio::Previewer;
use crate::autostart;
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

/// The Preview button: plays a short phrase with the sounds in force, and
/// returns once it has started. It changes no setting.
#[tauri::command(async)]
pub fn preview(
    gate: State<'_, Arc<CoreGate>>,
    previewer: State<'_, Arc<Previewer>>,
) -> Result<(), String> {
    previewer.start(gate.wait().live())
}

/// The "Import pack…" button: opens the system's folder picker, then
/// imports the folder chosen. `null` if the picker was cancelled. The
/// window never names a path: the folder comes from the picker alone.
#[tauri::command(async)]
pub fn import_pack(app: AppHandle, gate: State<'_, Arc<CoreGate>>) -> Result<Value, String> {
    let mut picker = app.dialog().file().set_title("Choose a sound pack folder");
    if let Some(window) = app.get_webview_window("main") {
        picker = picker.set_parent(&window);
    }
    // Blocks this command's thread, never the main one, until the user
    // chooses or cancels.
    let Some(path) = picker
        .blocking_pick_folder()
        .and_then(|chosen| chosen.into_path().ok())
    else {
        return Ok(Value::Null);
    };
    gate.wait().import_pack(&path)
}

/// Removes an imported pack. Bundled packs and unknown ids are refused.
#[tauri::command(async)]
pub fn remove_pack(gate: State<'_, Arc<CoreGate>>, id: String) -> Result<Value, String> {
    gate.wait().remove_pack(&id)
}

/// The "Open System Settings" button. `false` where there is no permission
/// to give. It does not wait at the gate: it needs nothing from the `Core`.
#[tauri::command(async)]
pub fn open_permission_settings(permission: State<'_, Arc<Permission>>) -> Result<bool, String> {
    permission::open_settings(&SystemAccess, &permission)
}

/// The "Restart now" button: quits and starts the app again. `false`, and
/// nothing happens, where there is no permission to wait for. Tauri hands
/// the old arguments to the new process, so the one that marks a login
/// start is taken out: the user is at the window and must get it back.
#[tauri::command(async)]
pub fn restart_app(app: AppHandle) -> bool {
    if !SystemAccess.required() {
        return false;
    }
    let restart = {
        let app = app.clone();
        move || {
            let mut env = app.env();
            env.args_os = autostart::without_hidden(env.args_os);
            // As Tauri's own restart does before it starts the new process:
            // this lets go of the single-instance lock, so the new copy is
            // not taken for a second launch and turned away.
            crate::tray::native::remove(&app);
            app.cleanup_before_exit();
            tauri::process::restart(&env);
        }
    };
    // The clean-up belongs on the main thread; a command runs off it.
    match app.run_on_main_thread(restart.clone()) {
        Ok(()) => loop {
            std::thread::sleep(std::time::Duration::MAX);
        },
        Err(_) => {
            restart();
            true
        }
    }
}
