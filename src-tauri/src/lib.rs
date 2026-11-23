pub mod audio;
pub mod keys;
pub mod packs;
pub mod startup;

use std::sync::{Arc, Mutex};

use tauri::Manager;

/// The product name shown in the window title and the UI.
pub const APP_NAME: &str = "Mechanical Key Sounds";

/// Builds the Tauri app and runs it until the last window is closed.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(start_clicks());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Opens the sound output and starts the key listener, each on its own
/// thread, so neither holds up the window. Nothing here can stop the app
/// from starting: with no output or no listener it runs without sound.
///
/// Until packs are chosen in settings, the pack folder comes from the
/// `MKS_PACK` environment variable; without it the built-in click plays.
fn start_clicks() -> audio::OutputHandle {
    let slot = Arc::new(audio::SinkSlot::new());
    let output = audio::spawn_output(Arc::clone(&slot));

    // Starting the listener can block briefly, and waiting on it blocks for
    // the life of the process. If the thread cannot be made, there are no
    // clicks.
    let _ = std::thread::Builder::new()
        .name("startup".into())
        .spawn(move || {
            let err: startup::ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
            // The pack is loaded before the listener starts, so no key plays
            // from a half-loaded pack.
            let pack = startup::pack_dir(std::env::var_os("MKS_PACK"));
            let sounds = startup::choose_sounds(pack.as_deref(), &err);
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
