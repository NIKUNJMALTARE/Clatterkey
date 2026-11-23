pub mod audio;
pub mod keys;
pub mod packs;
pub mod settings;
pub mod startup;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tauri::path::BaseDirectory;

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
            app.manage(start_clicks(packs, settings_path));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Opens the sound output and starts the key listener, each on its own
/// thread, so neither holds up the window. Nothing here can stop the app
/// from starting: with no output or no listener it runs without sound.
///
/// The pack comes from `pack_id` in the settings file, which is created
/// with defaults if missing; see `settings::load_or_create` and
/// `startup::choose_sounds` for the fallbacks.
fn start_clicks(packs: Option<PathBuf>, settings_path: Option<PathBuf>) -> audio::OutputHandle {
    let slot = Arc::new(audio::SinkSlot::new());
    let output = audio::spawn_output(Arc::clone(&slot));

    // Starting the listener can block briefly, and waiting on it blocks for
    // the life of the process. If the thread cannot be made, there are no
    // clicks.
    let _ = std::thread::Builder::new()
        .name("startup".into())
        .spawn(move || {
            let err: ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
            // The pack is loaded before the listener starts, so no key plays
            // from a half-loaded pack.
            let settings = match &settings_path {
                Some(path) => settings::load_or_create(path, &err),
                None => {
                    write_line(
                        &err,
                        "settings: the app config folder was not found; using defaults",
                    );
                    settings::Settings::default()
                }
            };
            let sounds = startup::choose_sounds(packs.as_deref(), &settings.pack_id, &err);
            startup::run_sounds(
                |consumer| {
                    let listener = keys::start(consumer)?;
                    Ok(Box::new(move || listener.wait()) as startup::Wait)
                },
                slot,
                sounds,
                err,
            );
        });

    output
}
