pub mod audio;
pub mod commands;
pub mod keys;
pub mod packs;
pub mod settings;
pub mod startup;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tauri::path::BaseDirectory;

use commands::CoreGate;

/// Where errors are reported: standard error in the app.
pub type ErrorOut = Arc<Mutex<dyn Write + Send>>;

/// Writes one line. Write errors are ignored: there is nowhere to report them.
pub fn write_line(out: &Mutex<dyn Write + Send>, line: &str) {
    let mut out = out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// The product name shown in the window title and the UI.
pub const APP_NAME: &str = "Mechanical Key Sounds";

/// Builds the Tauri app and runs it until the last window is closed.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Bundled packs are copied into the app's resource folder
            // (`bundle.resources` in `tauri.conf.json`).
            let packs = app
                .path()
                .resolve("resources/packs", BaseDirectory::Resource)
                .ok();
            let settings_path = app
                .path()
                .app_config_dir()
                .ok()
                .map(|dir| dir.join("settings.json"));
            // The window's commands wait at this gate until start-up has
            // read the settings and chosen the pack.
            let gate = Arc::new(CoreGate::new());
            app.manage(Arc::clone(&gate));
            app.manage(start_clicks(packs, settings_path, gate));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_packs,
            commands::get_settings,
            commands::set_settings,
            commands::audio_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Opens the sound output and starts the key listener, each on its own
/// thread, so neither holds up the window. Nothing here can stop the app
/// from starting: with no output or no listener it runs without sound.
///
/// The pack comes from `pack_id` in the settings file, which is created
/// with defaults if missing; see `settings::load_or_create_report` and
/// `startup::choose_pack` for the fallbacks. Once both are done the `Core`
/// is published to `gate` for the settings window, and only then does the
/// listener start.
fn start_clicks(
    packs: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    gate: Arc<CoreGate>,
) -> audio::OutputHandle {
    let slot = Arc::new(audio::SinkSlot::new());
    let output = audio::spawn_output(Arc::clone(&slot));

    // Starting the listener can block briefly, and waiting on it blocks for
    // the life of the process. If the thread cannot be made, there are no
    // clicks.
    let spawned = {
        let (gate, output) = (Arc::clone(&gate), output.clone());
        std::thread::Builder::new()
            .name("startup".into())
            .spawn(move || {
                let err: ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
                // The pack is loaded before the listener starts, so no key
                // plays from a half-loaded pack.
                let live = commands::publish_core(
                    &gate,
                    || status_of(&output),
                    &err,
                    |audio| commands::start_core(packs, settings_path, audio, &err),
                );
                startup::run_live(
                    |consumer| {
                        let listener = keys::start(consumer)?;
                        Ok(Box::new(move || listener.wait()) as startup::Wait)
                    },
                    slot,
                    live,
                    err,
                );
            })
    };
    if spawned.is_err() {
        // No clicks, but the window must still open with its controls.
        gate.publish(Arc::new(commands::plain_core(status_of(&output)).0));
    }

    output
}

fn status_of(output: &audio::OutputHandle) -> commands::AudioStatus {
    let output = output.clone();
    Box::new(move || output.status())
}
