//! Key groups: which kind of sound a key makes. This is the only place the
//! playback path looks at which key was pressed, and it gives back the group
//! and nothing else.

use super::KeyCode;

/// The five groups a sound pack can give its own sounds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyGroup {
    Default,
    Space,
    Enter,
    Backspace,
    Modifier,
}

impl KeyGroup {
    /// Every group, in `index` order.
    pub const ALL: [KeyGroup; 5] = [
        KeyGroup::Default,
        KeyGroup::Space,
        KeyGroup::Enter,
        KeyGroup::Backspace,
        KeyGroup::Modifier,
    ];

    /// The group's position in `ALL`, for array storage.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The group's name in `pack.json`.
    pub fn name(self) -> &'static str {
        match self {
            KeyGroup::Default => "default",
            KeyGroup::Space => "space",
            KeyGroup::Enter => "enter",
            KeyGroup::Backspace => "backspace",
            KeyGroup::Modifier => "modifier",
        }
    }

    /// The group with this `pack.json` name. Names are exact and lower case.
    pub fn from_name(name: &str) -> Option<KeyGroup> {
        KeyGroup::ALL.into_iter().find(|group| group.name() == name)
    }
}

/// The group a key's sound comes from.
pub fn group_of(code: KeyCode) -> KeyGroup {
    use KeyCode::*;
    match code {
        Space => KeyGroup::Space,
        Enter | NumpadEnter => KeyGroup::Enter,
        Backspace => KeyGroup::Backspace,
        ShiftLeft | ShiftRight | ControlLeft | ControlRight | AltLeft | AltRight | MetaLeft
        | MetaRight | CapsLock | Fn => KeyGroup::Modifier,
        KeyA | KeyB | KeyC | KeyD | KeyE | KeyF | KeyG | KeyH | KeyI | KeyJ | KeyK | KeyL
        | KeyM | KeyN | KeyO | KeyP | KeyQ | KeyR | KeyS | KeyT | KeyU | KeyV | KeyW | KeyX
        | KeyY | KeyZ | Digit0 | Digit1 | Digit2 | Digit3 | Digit4 | Digit5 | Digit6 | Digit7
        | Digit8 | Digit9 | Backquote | Minus | Equal | BracketLeft | BracketRight | Backslash
        | IntlBackslash | Semicolon | Quote | Comma | Period | Slash | Tab | Escape | Insert
        | Delete | ArrowUp | ArrowDown | ArrowLeft | ArrowRight | Home | End | PageUp
        | PageDown | F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 | F13 | F14
        | F15 | F16 | F17 | F18 | F19 | F20 | F21 | F22 | F23 | F24 | PrintScreen | ScrollLock
        | Pause | ContextMenu | NumLock | Numpad0 | Numpad1 | Numpad2 | Numpad3 | Numpad4
        | Numpad5 | Numpad6 | Numpad7 | Numpad8 | Numpad9 | NumpadAdd | NumpadSubtract
        | NumpadMultiply | NumpadDivide | NumpadDecimal | NumpadEqual | Unknown(_) => {
            KeyGroup::Default
        }
    }
}
