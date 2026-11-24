pub mod audio;
pub mod autostart;
pub mod commands;
pub mod handover;
pub mod keys;
pub mod packs;
pub mod permission;
pub mod settings;
pub mod startup;
pub mod tray;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

use commands::CoreGate;
use keys::permission::SystemAccess;
use permission::{Os, Permission, PermissionState, Watcher};
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
pub const APP_NAME: &str = "Clatterkey";

/// The event that tells the settings window of a change made anywhere,
/// with the `set_settings` result.
const SETTINGS_CHANGED: &str = "settings-changed";

/// Builds the Tauri app and runs it until the user quits. Closing the
/// settings window only hides it: the app lives on in the tray.
pub fn run() {
    let builder = tauri::Builder::default()
        // Registered first, as the plugin asks. A second launch starts
        // nothing: the running app shows its settings window.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if autostart::show_on_second_launch(&args) {
                tray::native::show_settings(app);
            }
        }))
        // The folder picker for importing a pack. Only the core opens it:
        // the window is given no permission to.
        .plugin(tauri_plugin_dialog::init());
    // The system's login registration. A development build has none, so
    // that a build folder is never left in the user's login items.
    let builder = if autostart::native::registers() {
        builder.plugin(autostart::native::plugin())
    } else {
        builder
    };
    let app = builder
        .setup(|app| {
            // Bundled packs are copied into the app's resource folder
            // (`bundle.resources` in `tauri.conf.json`).
            let packs = app
                .path()
                .resolve("resources/packs", BaseDirectory::Resource)
                .ok();
            // Imported packs are copied here; the first import makes it.
            let imported = app.path().app_data_dir().ok().map(|dir| dir.join("packs"));
            let library = packs::Library::new(packs, imported);
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
            // The window is made hidden. A start by hand shows it now; a
            // start made by the login registration leaves it hidden until
            // the permission is known, and shows it only if it is missing
            // or there is no tray icon to reach the app by.
            let login_start = autostart::is_login_start(&std::env::args_os().collect::<Vec<_>>());
            let decide: StartDecision = if login_start {
                #[cfg(target_os = "macos")]
                let _ = app
                    .handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
                let handle = app.handle().clone();
                Box::new(move |permission| {
                    if autostart::show_at_start(true, permission, tray_present(&handle)) {
                        tray::native::show_settings(&handle);
                    }
                })
            } else {
                tray::native::show_settings(app.handle());
                Box::new(|_| {})
            };
            let login_item = autostart::native::login_item(app.handle());
            // On macOS the listener starts only once Input Monitoring is
            // granted; this is where the window reads how that stands.
            let permission = Arc::new(Permission::new(permission::initial_state(&SystemAccess)));
            app.manage(Arc::clone(&permission));
            // Key presses and the Preview button play into the same slot.
            let slot = Arc::new(audio::SinkSlot::new());
            app.manage(Arc::new(audio::Previewer::new(
                Arc::clone(&slot) as Arc<dyn audio::Sink>
            )));
            app.manage(start_clicks(
                library,
                settings_path,
                gate,
                permission,
                slot,
                login_item,
                decide,
            ));
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
            commands::import_pack,
            commands::remove_pack,
            commands::audio_status,
            commands::preview,
            commands::permission_status,
            commands::open_permission_settings,
            commands::restart_app,
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

/// Told the permission state once start-up knows it; shows the settings
/// window after a login start if it has to be shown. Called exactly once.
type StartDecision = Box<dyn FnOnce(PermissionState) + Send>;

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
/// listener start. Where the OS has a key-listening permission (macOS), the
/// listener starts when it is granted, and this thread goes on watching it.
fn start_clicks(
    library: packs::Library,
    settings_path: Option<PathBuf>,
    gate: Arc<CoreGate>,
    permission: Arc<Permission>,
    slot: Arc<audio::SinkSlot>,
    login_item: Arc<dyn autostart::LoginItem>,
    decide: StartDecision,
) -> audio::OutputHandle {
    let output = audio::spawn_output(Arc::clone(&slot));
    // Taken by whichever path gets to decide: the start-up thread, or
    // this one if that thread cannot be made.
    let decide = Arc::new(Mutex::new(Some(decide)));
    let decided = |state: PermissionState, decide: &Mutex<Option<StartDecision>>| {
        let taken = decide.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(decide) = taken {
            decide(state);
        }
    };

    // Starting the listener can block briefly, and waiting on it blocks for
    // the life of the process. If the thread cannot be made, there are no
    // clicks.
    let spawned = {
        let (gate, output) = (Arc::clone(&gate), output.clone());
        let permission = Arc::clone(&permission);
        let decide = Arc::clone(&decide);
        std::thread::Builder::new()
            .name("startup".into())
            .spawn(move || {
                let err: ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
                // The pack is loaded before the listener starts, so no key
                // plays from a half-loaded pack.
                //
                // What the app left under an earlier name is carried over
                // first, and the order matters: the earlier settings and
                // packs are copied before the settings are read (a first
                // read would write the defaults and the copy would never
                // be made); then, with the setting known, the earlier
                // login registrations are taken out and the new one put in
                // their place; only then is the setting compared with the
                // system.
                let build = |audio| {
                    let copied = match settings_path.as_deref().and_then(Path::parent) {
                        Some(folder) => {
                            handover::copy_files(&handover::earlier_folders(folder), folder, &err)
                        }
                        None => handover::Copied::Nothing,
                    };
                    let new_item = Arc::clone(&login_item);
                    commands::start_core_after(
                        library,
                        settings_path,
                        audio,
                        Arc::clone(&permission),
                        login_item,
                        &err,
                        |setting_on| {
                            // In the order of the folders above, so that
                            // the copy's position names the same app here.
                            let earlier: Vec<_> = handover::EARLIER
                                .iter()
                                .map(|earlier| autostart::native::old_login_item(earlier.app_name))
                                .collect();
                            let earlier: Vec<&dyn autostart::LoginItem> =
                                earlier.iter().map(|item| &**item).collect();
                            handover::login(
                                copied,
                                setting_on,
                                autostart::native::registers(),
                                &earlier,
                                &*new_item,
                                &err,
                            );
                        },
                    )
                };
                if SystemAccess.required() {
                    // The first check runs before the window's commands are
                    // let through, so its first read has the true state and
                    // it never shows the permission screen by mistake.
                    let mut watcher = None;
                    commands::publish_core_after(
                        &gate,
                        || status_of(&output),
                        Arc::clone(&permission),
                        &err,
                        build,
                        |live| {
                            let keys = startup::LiveKeys::new(
                                start_listener,
                                slot,
                                Arc::clone(live),
                                Arc::clone(&err),
                            );
                            let mut first = Watcher::new(
                                Arc::new(SystemAccess),
                                keys,
                                Arc::clone(&permission),
                                Arc::clone(&err),
                            );
                            first.step();
                            watcher = Some(first);
                        },
                    );
                    match watcher {
                        Some(watcher) => {
                            decided(permission.state(), &decide);
                            std::thread::sleep(permission::CHECK_WHILE_MISSING);
                            watcher.watch(std::thread::sleep);
                        }
                        // The first check failed, so nothing will listen.
                        None => {
                            permission.set(PermissionState::RestartNeeded);
                            decided(PermissionState::RestartNeeded, &decide);
                        }
                    }
                } else {
                    let live = commands::publish_core(
                        &gate,
                        || status_of(&output),
                        Arc::clone(&permission),
                        &err,
                        build,
                    );
                    decided(permission.state(), &decide);
                    startup::run_live(
                        |consumer| {
                            let listener = keys::start(consumer)?;
                            Ok(Box::new(move || listener.wait()) as startup::Wait)
                        },
                        slot,
                        live,
                        err,
                    );
                }
            })
    };
    if spawned.is_err() {
        // Nothing will listen or watch the permission in this run, so the
        // window must not promise that granting it starts the sound.
        if SystemAccess.required() {
            permission.set(PermissionState::RestartNeeded);
        }
        // No clicks, but the window must still open with its controls.
        gate.publish(Arc::new(
            commands::plain_core(status_of(&output), Arc::clone(&permission)).0,
        ));
        // The window must not stay hidden with nothing behind it.
        decided(permission.state(), &decide);
    }

    output
}

/// Starts the real key listener for the permission watcher, which may stop
/// it and start another later.
fn start_listener(consumer: startup::Consumer) -> Result<startup::Running, keys::KeyListenError> {
    let listener = Arc::new(Mutex::new(Some(keys::start(consumer)?)));
    let held = Arc::clone(&listener);
    Ok(startup::Running {
        stop: Box::new(move || {
            let taken = held.lock().unwrap_or_else(|p| p.into_inner()).take();
            if let Some(listener) = taken {
                listener.stop();
            }
        }),
        finished: Box::new(move || {
            listener
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .is_none_or(keys::Listener::is_finished)
        }),
    })
}

fn status_of(output: &audio::OutputHandle) -> commands::AudioStatus {
    let output = output.clone();
    Box::new(move || output.status())
}
