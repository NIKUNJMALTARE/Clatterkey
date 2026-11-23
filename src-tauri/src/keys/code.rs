//! The app's own key codes. They name physical keys (the position on a
//! QWERTY keyboard), never the character a key types, and follow the W3C
//! `KeyboardEvent.code` names.

use std::fmt;

/// Declares `KeyCode` and its `Display` names from one list, so a variant
/// cannot be added without a name.
macro_rules! key_codes {
    ($($name:ident),* $(,)?) => {
        /// One physical key. `Unknown` carries the raw OS number of a key
        /// that has no name here.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum KeyCode {
            $($name,)*
            Unknown(u32),
        }

        impl fmt::Display for KeyCode {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(KeyCode::$name => f.write_str(stringify!($name)),)*
                    KeyCode::Unknown(raw) => write!(f, "Unknown({raw})"),
                }
            }
        }
    };
}

key_codes! {
    // Letters
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
    KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    // Digit row
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    // Punctuation
    Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash, IntlBackslash,
    Semicolon, Quote, Comma, Period, Slash,
    // Editing and whitespace
    Space, Enter, Backspace, Tab, Escape, Insert, Delete,
    // Navigation
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Home, End, PageUp, PageDown,
    // Modifiers
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight,
    MetaLeft, MetaRight, CapsLock, Fn,
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    // System
    PrintScreen, ScrollLock, Pause, ContextMenu,
    // Numpad
    NumLock, Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6,
    Numpad7, Numpad8, Numpad9, NumpadAdd, NumpadSubtract, NumpadMultiply,
    NumpadDivide, NumpadDecimal, NumpadEnter, NumpadEqual,
}
