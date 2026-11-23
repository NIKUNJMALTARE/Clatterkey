//! The tray icon and its menu: sound on or off, open the settings, quit.
//!
//! `model` decides what the tray shows and does and is plain Rust; `native`
//! is the only code that touches Tauri's tray, menu and window types.

mod model;
pub mod native;

pub use model::{
    CloseDecision, ExitDecision, MENU, MenuEntry, SOUND_ID, TrayAction, TrayIconKind, TraySync,
    TrayView, action_for, on_close_request, on_exit_request, toggle_sound,
};
