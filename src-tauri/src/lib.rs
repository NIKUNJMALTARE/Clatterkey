pub mod audio;
pub mod commands;
pub mod keys;
pub mod packs;
pub mod settings;
pub mod startup;
pub mod tray;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

use commands::CoreGate;
use tray::native::TrayPresent;
use tray::{CloseDecision, ExitDecision, TraySync};

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

/// The event that tells the settings window of a change made anywhere,
/// with the `set_settings` result.
const SETTINGS_CHANGED: &str = "settings-changed";

/// Builds the Tauri app and runs it until the user quits. Closing the
/// settings window only hides it: the app lives on in the tray.
pub fn run() {
    let app = tauri::Builder::default()
        // Registered first, as the plugin asks. A second launch starts
        // nothing: the running app shows its settings window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::native::show_settings(app);
        }))
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
            start_tray(app.handle(), Arc::clone(&gate));
            app.manage(start_clicks(packs, settings_path, gate));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && tray::on_close_request(tray_present(window.app_handle())) == CloseDecision::Hide
            {
                api.prevent_close();
                tray::native::hide_settings(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_packs,
            commands::get_settings,
            commands::set_settings,
            commands::audio_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|app, event| match event {
        // With no code the last window went away; with one, the user chose
        // Quit (or Cmd+Q), and that always goes through.
        RunEvent::ExitRequested { code, api, .. } => {
            if tray::on_exit_request(code.is_some(), tray_present(app)) == ExitDecision::Stay {
                api.prevent_exit();
            }
        }
        // Opening the app again from Finder, Launchpad or the Dock.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tray::native::show_settings(app),
        RunEvent::Exit => tray::native::remove(app),
        _ => {}
    });
}

fn tray_present(app: &AppHandle) -> bool {
    app.try_state::<TrayPresent>().is_some_and(|tray| tray.0)
}

/// Makes the tray icon, and keeps it and the settings window in step with
/// the settings from then on. If the icon cannot be made the app still
/// runs, and closing the window quits it as before.
fn start_tray(app: &AppHandle, gate: Arc<CoreGate>) {
    let built = tray::native::build(app);
    if let Err(error) = &built {
        let err: ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
        write_line(
            &err,
            &format!("tray: the icon could not be made: {error}; closing the window will quit"),
        );
    }
    app.manage(TrayPresent(built.is_ok()));
    let sync = Arc::new(TraySync::new());
    app.manage(Arc::clone(&sync));

    // Subscribing waits for start-up to publish the core, so it is done
    // off the main thread. The listener only queues work: an event for the
    // window and a main-thread update of the tray.
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("tray-sync".into())
        .spawn(move || {
            let core = gate.wait();
            core.subscribe(Box::new(move |result| {
                let _ = app.emit(SETTINGS_CHANGED, result);
                if let Some(view) = sync.update(result) {
                    tray::native::apply(&app, view);
                }
            }));
        });
    if let Err(error) = spawned {
        let err: ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
        write_line(
            &err,
            &format!(
                "tray: could not follow the settings: {error}; the icon and the window may not show changes"
            ),
        );
    }
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
