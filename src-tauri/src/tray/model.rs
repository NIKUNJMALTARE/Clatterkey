//! What the tray shows and does, with no Tauri in it, so it is tested
//! directly. `native` carries out what is decided here.

use std::sync::Mutex;

use serde_json::Value;

use crate::commands::CoreGate;
use crate::{APP_NAME, ErrorOut, write_line};

/// Which of the two tray icons is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconKind {
    /// Sound is on.
    Normal,
    /// Sound is off: the dimmed icon.
    Off,
}

/// Everything about the tray that follows the `enabled` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayView {
    /// Whether the "Sound" item is ticked.
    pub checked: bool,
    pub icon: TrayIconKind,
    pub tooltip: &'static str,
}

impl TrayView {
    /// The tray for sound on or off.
    pub fn of(enabled: bool) -> Self {
        if enabled {
            Self {
                checked: true,
                icon: TrayIconKind::Normal,
                tooltip: APP_NAME,
            }
        } else {
            Self {
                checked: false,
                icon: TrayIconKind::Off,
                tooltip: "ClickClak (sound off)",
            }
        }
    }
}

/// What a menu item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ToggleSound,
    ShowSettings,
    Quit,
}

/// One row of the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
    Item {
        id: &'static str,
        label: &'static str,
        action: TrayAction,
    },
    Separator,
}

/// The id of the ticked "Sound" item.
pub const SOUND_ID: &str = "sound";

/// The tray menu, top to bottom.
pub const MENU: [MenuEntry; 4] = [
    MenuEntry::Item {
        id: SOUND_ID,
        label: "Sound",
        action: TrayAction::ToggleSound,
    },
    MenuEntry::Item {
        id: "settings",
        label: "Settings\u{2026}",
        action: TrayAction::ShowSettings,
    },
    MenuEntry::Separator,
    MenuEntry::Item {
        id: "quit",
        label: "Quit ClickClak",
        action: TrayAction::Quit,
    },
];

/// The action of the menu item with this id.
pub fn action_for(id: &str) -> Option<TrayAction> {
    MENU.iter().find_map(|entry| match entry {
        MenuEntry::Item {
            id: item, action, ..
        } if *item == id => Some(*action),
        _ => None,
    })
}

/// Remembers the view the tray last showed, so the tray is touched only
/// when `enabled` changes, not on every volume or pack change.
#[derive(Debug, Default)]
pub struct TraySync {
    last: Mutex<Option<TrayView>>,
}

impl TraySync {
    pub fn new() -> Self {
        Self::default()
    }

    /// The view to show after the change `result` (a `set_settings`
    /// result), or `None` if the tray already shows it. The first call
    /// always gives a view.
    pub fn update(&self, result: &Value) -> Option<TrayView> {
        let enabled = result["settings"]["enabled"].as_bool()?;
        let view = TrayView::of(enabled);
        let mut last = self.last.lock().unwrap_or_else(|p| p.into_inner());
        if *last == Some(view) {
            return None;
        }
        *last = Some(view);
        Some(view)
    }

    /// The view last given by `update`, if any.
    pub fn current(&self) -> Option<TrayView> {
        *self.last.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// The "Sound" menu item: waits until start-up has published the core,
/// flips `enabled` and saves. The change is in force even if the save
/// fails; that is written to `err` once and is in the result for the
/// window. This can wait and write a file, so it is not for the main
/// thread.
pub fn toggle_sound(gate: &CoreGate, err: &ErrorOut) -> Value {
    let result = gate.wait().toggle_enabled();
    if result["saved"] == false {
        let reason = result["save_error"].as_str().unwrap_or("unknown error");
        write_line(err, &format!("settings: could not save: {reason}"));
    }
    result
}

/// What to do when the user closes the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// Keep running in the tray.
    Hide,
    /// Let the window close, which ends the app.
    Close,
}

/// With a tray icon the window hides; without one the app would be out of
/// reach, so the window closes as it did before there was a tray.
pub fn on_close_request(tray_exists: bool) -> CloseDecision {
    if tray_exists {
        CloseDecision::Hide
    } else {
        CloseDecision::Close
    }
}

/// What to do when the app is asked to exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitDecision {
    Stay,
    Exit,
}

/// The app stays only when nobody asked to quit (the last window went
/// away) and there is a tray icon to reach it by.
pub fn on_exit_request(quit_asked: bool, tray_exists: bool) -> ExitDecision {
    if !quit_asked && tray_exists {
        ExitDecision::Stay
    } else {
        ExitDecision::Exit
    }
}
