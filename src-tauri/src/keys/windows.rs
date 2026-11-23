//! Windows back end: `rdev`'s low-level keyboard hook. No `rdev` type leaves
//! this file.

use std::panic::{AssertUnwindSafe, catch_unwind};

use rdev::{EventType, Key};

use super::{KeyCode, KeyListenError, RawKey, Started};

/// Start of the numbers given to keys `rdev` names but this app does not.
const MEDIA_BASE: u32 = 0x1_0000;

/// Maps an `rdev` key to the app's code. Keys with no name here keep a raw
/// number as `Unknown`.
fn code_from_rdev(key: Key) -> KeyCode {
    use KeyCode as C;
    match key {
        Key::KeyA => C::KeyA,
        Key::KeyB => C::KeyB,
        Key::KeyC => C::KeyC,
        Key::KeyD => C::KeyD,
        Key::KeyE => C::KeyE,
        Key::KeyF => C::KeyF,
        Key::KeyG => C::KeyG,
        Key::KeyH => C::KeyH,
        Key::KeyI => C::KeyI,
        Key::KeyJ => C::KeyJ,
        Key::KeyK => C::KeyK,
        Key::KeyL => C::KeyL,
        Key::KeyM => C::KeyM,
        Key::KeyN => C::KeyN,
        Key::KeyO => C::KeyO,
        Key::KeyP => C::KeyP,
        Key::KeyQ => C::KeyQ,
        Key::KeyR => C::KeyR,
        Key::KeyS => C::KeyS,
        Key::KeyT => C::KeyT,
        Key::KeyU => C::KeyU,
        Key::KeyV => C::KeyV,
        Key::KeyW => C::KeyW,
        Key::KeyX => C::KeyX,
        Key::KeyY => C::KeyY,
        Key::KeyZ => C::KeyZ,
        Key::Num0 => C::Digit0,
        Key::Num1 => C::Digit1,
        Key::Num2 => C::Digit2,
        Key::Num3 => C::Digit3,
        Key::Num4 => C::Digit4,
        Key::Num5 => C::Digit5,
        Key::Num6 => C::Digit6,
        Key::Num7 => C::Digit7,
        Key::Num8 => C::Digit8,
        Key::Num9 => C::Digit9,
        Key::BackQuote => C::Backquote,
        Key::Minus => C::Minus,
        Key::Equal => C::Equal,
        Key::LeftBracket => C::BracketLeft,
        Key::RightBracket => C::BracketRight,
        Key::BackSlash => C::Backslash,
        Key::IntlBackslash => C::IntlBackslash,
        Key::SemiColon => C::Semicolon,
        Key::Quote => C::Quote,
        Key::Comma => C::Comma,
        Key::Dot => C::Period,
        Key::Slash => C::Slash,
        Key::Space => C::Space,
        Key::Return => C::Enter,
        Key::Backspace => C::Backspace,
        Key::Tab => C::Tab,
        Key::Escape => C::Escape,
        Key::Insert => C::Insert,
        Key::Delete => C::Delete,
        Key::UpArrow => C::ArrowUp,
        Key::DownArrow => C::ArrowDown,
        Key::LeftArrow => C::ArrowLeft,
        Key::RightArrow => C::ArrowRight,
        Key::Home => C::Home,
        Key::End => C::End,
        Key::PageUp => C::PageUp,
        Key::PageDown => C::PageDown,
        Key::ShiftLeft => C::ShiftLeft,
        Key::ShiftRight => C::ShiftRight,
        Key::ControlLeft => C::ControlLeft,
        Key::ControlRight => C::ControlRight,
        Key::Alt => C::AltLeft,
        Key::AltGr => C::AltRight,
        Key::MetaLeft => C::MetaLeft,
        Key::MetaRight => C::MetaRight,
        Key::CapsLock => C::CapsLock,
        Key::Function => C::Fn,
        Key::F1 => C::F1,
        Key::F2 => C::F2,
        Key::F3 => C::F3,
        Key::F4 => C::F4,
        Key::F5 => C::F5,
        Key::F6 => C::F6,
        Key::F7 => C::F7,
        Key::F8 => C::F8,
        Key::F9 => C::F9,
        Key::F10 => C::F10,
        Key::F11 => C::F11,
        Key::F12 => C::F12,
        Key::F13 => C::F13,
        Key::F14 => C::F14,
        Key::F15 => C::F15,
        Key::F16 => C::F16,
        Key::F17 => C::F17,
        Key::F18 => C::F18,
        Key::F19 => C::F19,
        Key::F20 => C::F20,
        Key::F21 => C::F21,
        Key::F22 => C::F22,
        Key::F23 => C::F23,
        Key::F24 => C::F24,
        Key::PrintScreen => C::PrintScreen,
        Key::ScrollLock => C::ScrollLock,
        Key::Pause => C::Pause,
        Key::NumLock => C::NumLock,
        Key::Kp0 => C::Numpad0,
        Key::Kp1 => C::Numpad1,
        Key::Kp2 => C::Numpad2,
        Key::Kp3 => C::Numpad3,
        Key::Kp4 => C::Numpad4,
        Key::Kp5 => C::Numpad5,
        Key::Kp6 => C::Numpad6,
        Key::Kp7 => C::Numpad7,
        Key::Kp8 => C::Numpad8,
        Key::Kp9 => C::Numpad9,
        Key::KpPlus => C::NumpadAdd,
        Key::KpMinus => C::NumpadSubtract,
        Key::KpMultiply => C::NumpadMultiply,
        Key::KpDivide => C::NumpadDivide,
        Key::KpDelete => C::NumpadDecimal,
        Key::KpReturn => C::NumpadEnter,
        Key::Unknown(raw) => C::Unknown(raw),
        // Media keys have no name here. They are still reported, each with
        // its own number in a range no real key code uses.
        Key::VolumeUp => C::Unknown(MEDIA_BASE + 1),
        Key::VolumeDown => C::Unknown(MEDIA_BASE + 2),
        Key::VolumeMute => C::Unknown(MEDIA_BASE + 3),
        Key::BrightnessUp => C::Unknown(MEDIA_BASE + 4),
        Key::BrightnessDown => C::Unknown(MEDIA_BASE + 5),
        Key::PreviousTrack => C::Unknown(MEDIA_BASE + 6),
        Key::PlayPause => C::Unknown(MEDIA_BASE + 7),
        Key::PlayCd => C::Unknown(MEDIA_BASE + 8),
        Key::NextTrack => C::Unknown(MEDIA_BASE + 9),
        // Anything a later `rdev` adds.
        #[allow(unreachable_patterns)]
        _ => C::Unknown(MEDIA_BASE),
    }
}

/// Turns an `rdev` event into a raw key, or `None` for mouse events.
fn raw_from_rdev(event: &EventType) -> Option<RawKey> {
    match event {
        EventType::KeyPress(key) => Some(RawKey::Press(code_from_rdev(*key))),
        EventType::KeyRelease(key) => Some(RawKey::Release(code_from_rdev(*key))),
        _ => None,
    }
}

/// Runs the hook on the calling thread until it ends.
///
/// `rdev` gives no signal that the hook is installed: it either returns an
/// error at once or blocks. So nothing is sent through `started` on success;
/// `start` takes a short silence to mean the hook is in. A failure is both
/// sent and returned. `started` is kept until the end so that silence is not
/// mistaken for the thread having ended.
pub(super) fn run(
    mut deliver: impl FnMut(RawKey) + 'static,
    started: Started,
) -> Result<(), KeyListenError> {
    // `rdev` calls the closure from an `extern "system"` hook, which a panic
    // must not unwind through (the process would abort). After a panic in
    // the consumer, events are dropped: `rdev` has no way to stop its loop.
    let mut consumer_failed = false;
    let listened = rdev::listen(move |event| {
        if consumer_failed {
            return;
        }
        // Only the key is read. `event.name`, the typed character, is never
        // touched.
        if let Some(raw) = raw_from_rdev(&event.event_type) {
            consumer_failed = catch_unwind(AssertUnwindSafe(|| deliver(raw))).is_err();
        }
    });
    let result = match listened {
        Err(error) => Err(KeyListenError::Start(format!(
            "Windows refused the keyboard hook ({error:?})"
        ))),
        // The hook's message loop is meant to run for the life of the
        // process, so its ending is a failure too.
        Ok(()) => Err(KeyListenError::Stopped(
            "the Windows hook loop ended".into(),
        )),
    };
    if let Err(error) = &result {
        let _ = started.send(Err(error.clone()));
    }
    result
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
