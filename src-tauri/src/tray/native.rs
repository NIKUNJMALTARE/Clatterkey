//! The tray icon, its menu and the settings window's show and hide, in
//! Tauri's types. Everything decided here is decided by `model`; this only
//! carries it out. It cannot run in tests, so it is kept thin.

use std::sync::{Arc, Mutex};

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use super::model::{
    MENU, MenuEntry, SOUND_ID, TrayAction, TrayIconKind, TrayView, action_for, toggle_sound,
};
use crate::ErrorOut;
use crate::commands::CoreGate;

/// The tray icon's id, and the settings window's label.
const TRAY_ID: &str = "main";
const WINDOW: &str = "main";

/// Whether the tray icon was made. Without it the app must not outlive its
/// window: see `model::on_close_request`.
pub struct TrayPresent(pub bool);

/// The ticked "Sound" item, kept so `apply` can set its tick.
struct SoundItem(CheckMenuItem<Wry>);

// The icons are drawn by `examples/make_tray_icons`. macOS gets one-colour
// template images; Windows and others get coloured ones.
#[cfg(target_os = "macos")]
const ICON_NORMAL: &[u8] = include_bytes!("../../icons/tray/mac.png");
#[cfg(target_os = "macos")]
const ICON_OFF: &[u8] = include_bytes!("../../icons/tray/mac-off.png");
#[cfg(not(target_os = "macos"))]
const ICON_NORMAL: &[u8] = include_bytes!("../../icons/tray/win.png");
#[cfg(not(target_os = "macos"))]
const ICON_OFF: &[u8] = include_bytes!("../../icons/tray/win-off.png");

fn icon(kind: TrayIconKind) -> tauri::Result<Image<'static>> {
    Image::from_bytes(match kind {
        TrayIconKind::Normal => ICON_NORMAL,
        TrayIconKind::Off => ICON_OFF,
    })
}

/// Makes the tray icon and its menu, showing sound as on until the
/// settings are known. Call it from `setup`, on the main thread.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let view = TrayView::of(true);
    let sound = CheckMenuItem::with_id(app, SOUND_ID, "Sound", true, view.checked, None::<&str>)?;

    let mut built: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for entry in &MENU {
        built.push(match entry {
            MenuEntry::Item { id, .. } if *id == SOUND_ID => Box::new(sound.clone()),
            MenuEntry::Item { id, label, .. } => {
                Box::new(MenuItem::with_id(app, *id, *label, true, None::<&str>)?)
            }
            MenuEntry::Separator => Box::new(PredefinedMenuItem::separator(app)?),
        });
    }
    let items: Vec<&dyn IsMenuItem<Wry>> = built.iter().map(|item| item.as_ref()).collect();
    let menu = Menu::with_items(app, &items)?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(view.icon)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(view.tooltip)
        .menu(&menu)
        // macOS opens the menu on any click. Elsewhere a left click opens
        // the settings and a right click the menu.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| {
            if let Some(action) = action_for(event.id().as_ref()) {
                run(app, action);
            }
        });
    #[cfg(not(target_os = "macos"))]
    let tray = tray.on_tray_icon_event(|tray, event| {
        use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            show_settings(tray.app_handle());
        }
    });
    tray.build(app)?;

    app.manage(SoundItem(sound));
    Ok(())
}

/// Carries out a menu item. Called on the main thread.
fn run(app: &AppHandle, action: TrayAction) {
    match action {
        TrayAction::ToggleSound => {
            // The toggle waits for start-up and writes the settings file,
            // so it runs on a thread of its own. The tray is brought into
            // step by the change listener, whichever side made the change.
            let gate = Arc::clone(&app.state::<Arc<CoreGate>>());
            let spawned = std::thread::Builder::new()
                .name("tray-toggle".into())
                .spawn(move || {
                    let err: ErrorOut = Arc::new(Mutex::new(std::io::stderr()));
                    toggle_sound(&gate, &err);
                });
            if spawned.is_err() {
                // Nothing changed; put back the tick the OS flipped.
                resync(app);
            }
        }
        TrayAction::ShowSettings => show_settings(app),
        // An exit with a code is a quit that was asked for, so
        // `model::on_exit_request` lets it through.
        TrayAction::Quit => app.exit(0),
    }
}

/// Sets the tick back to what the icon's state says, after a click that
/// changed nothing.
fn resync(app: &AppHandle) {
    if let Some(sync) = app.try_state::<Arc<super::TraySync>>()
        && let Some(view) = sync.current()
    {
        apply(app, view);
    }
}

/// Shows `view` in the tray. Safe from any thread: the work is done on the
/// main thread, and never on the key listener's.
pub fn apply(app: &AppHandle, view: TrayView) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        // The OS flips the item's tick itself when it is clicked; this
        // sets it to the setting in force.
        if let Some(item) = handle.try_state::<SoundItem>() {
            let _ = item.0.set_checked(view.checked);
        }
        if let Some(tray) = handle.tray_by_id(TRAY_ID) {
            if let Ok(image) = icon(view.icon) {
                let _ = tray.set_icon(Some(image));
                // Setting an icon clears the template flag on macOS.
                let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
            }
            let _ = tray.set_tooltip(Some(view.tooltip));
        }
    });
}

/// Shows the settings window and brings it to the front: restored if it
/// was minimised, and with its Dock icon back on macOS.
pub fn show_settings(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let _ = handle.set_activation_policy(tauri::ActivationPolicy::Regular);
        if let Some(window) = handle.get_webview_window(WINDOW) {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

/// Hides the settings window; the app carries on in the tray. On macOS the
/// Dock icon goes with it.
pub fn hide_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

/// Takes the icon out of the tray as the app exits, so Windows does not
/// keep a dead one until the pointer passes over it.
pub fn remove(app: &AppHandle) {
    let _ = app.remove_tray_by_id(TRAY_ID);
}
