//! macOS back end: a listen-only, keyboard-only event tap.
//!
//! It reads the virtual key code and the modifier flags of each event and
//! nothing else. It makes no keyboard-layout or text-input calls, so it never
//! learns the typed character and is safe to run off the main thread.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::NonNull;

use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventMask, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};

use super::{KeyCode, KeyListenError, RawKey, Started};

// CGEventType numbers.
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

/// Only these three event types are asked for, so mouse events never reach
/// this process.
const KEYBOARD_EVENTS: CGEventMask = (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << FLAGS_CHANGED);

// CGEventFlags bits, one per kind of modifier.
const FLAG_CAPS_LOCK: u64 = 0x0001_0000;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_ALT: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;
const FLAG_FN: u64 = 0x0080_0000;

/// The flag bit a modifier key sets, or `None` if the virtual key is not a
/// modifier.
fn modifier_flag(keycode: u16) -> Option<u64> {
    match keycode {
        54 | 55 => Some(FLAG_COMMAND),
        56 | 60 => Some(FLAG_SHIFT),
        57 => Some(FLAG_CAPS_LOCK),
        58 | 61 => Some(FLAG_ALT),
        59 | 62 => Some(FLAG_CONTROL),
        63 => Some(FLAG_FN),
        _ => None,
    }
}

/// Which modifier keys are down. macOS reports modifiers as "the flags
/// changed" rather than as presses and releases, and the left and right keys
/// of a pair share one flag, so each key has to be remembered.
#[derive(Debug, Default)]
struct ModifierState {
    held: Vec<u16>,
    /// The caps-lock state last seen, or `None` before the first event.
    caps_lock_on: Option<bool>,
}

impl ModifierState {
    /// Records the key as down. Returns false if it already was.
    fn press(&mut self, keycode: u16) -> bool {
        if self.held.contains(&keycode) {
            return false;
        }
        self.held.push(keycode);
        true
    }

    fn release(&mut self, keycode: u16) {
        self.held.retain(|held| *held != keycode);
    }
}

/// Maps a macOS virtual key code (the `kVK_` constants) to the app's code.
fn code_from_keycode(keycode: u16) -> KeyCode {
    use KeyCode as C;
    match keycode {
        0 => C::KeyA,
        1 => C::KeyS,
        2 => C::KeyD,
        3 => C::KeyF,
        4 => C::KeyH,
        5 => C::KeyG,
        6 => C::KeyZ,
        7 => C::KeyX,
        8 => C::KeyC,
        9 => C::KeyV,
        10 => C::IntlBackslash,
        11 => C::KeyB,
        12 => C::KeyQ,
        13 => C::KeyW,
        14 => C::KeyE,
        15 => C::KeyR,
        16 => C::KeyY,
        17 => C::KeyT,
        18 => C::Digit1,
        19 => C::Digit2,
        20 => C::Digit3,
        21 => C::Digit4,
        22 => C::Digit6,
        23 => C::Digit5,
        24 => C::Equal,
        25 => C::Digit9,
        26 => C::Digit7,
        27 => C::Minus,
        28 => C::Digit8,
        29 => C::Digit0,
        30 => C::BracketRight,
        31 => C::KeyO,
        32 => C::KeyU,
        33 => C::BracketLeft,
        34 => C::KeyI,
        35 => C::KeyP,
        36 => C::Enter,
        37 => C::KeyL,
        38 => C::KeyJ,
        39 => C::Quote,
        40 => C::KeyK,
        41 => C::Semicolon,
        42 => C::Backslash,
        43 => C::Comma,
        44 => C::Slash,
        45 => C::KeyN,
        46 => C::KeyM,
        47 => C::Period,
        48 => C::Tab,
        49 => C::Space,
        50 => C::Backquote,
        51 => C::Backspace,
        53 => C::Escape,
        54 => C::MetaRight,
        55 => C::MetaLeft,
        56 => C::ShiftLeft,
        57 => C::CapsLock,
        58 => C::AltLeft,
        59 => C::ControlLeft,
        60 => C::ShiftRight,
        61 => C::AltRight,
        62 => C::ControlRight,
        63 => C::Fn,
        64 => C::F17,
        65 => C::NumpadDecimal,
        67 => C::NumpadMultiply,
        69 => C::NumpadAdd,
        71 => C::NumLock,
        75 => C::NumpadDivide,
        76 => C::NumpadEnter,
        78 => C::NumpadSubtract,
        79 => C::F18,
        80 => C::F19,
        81 => C::NumpadEqual,
        82 => C::Numpad0,
        83 => C::Numpad1,
        84 => C::Numpad2,
        85 => C::Numpad3,
        86 => C::Numpad4,
        87 => C::Numpad5,
        88 => C::Numpad6,
        89 => C::Numpad7,
        90 => C::F20,
        91 => C::Numpad8,
        92 => C::Numpad9,
        96 => C::F5,
        97 => C::F6,
        98 => C::F7,
        99 => C::F3,
        100 => C::F8,
        101 => C::F9,
        103 => C::F11,
        105 => C::F13,
        106 => C::F16,
        107 => C::F14,
        109 => C::F10,
        110 => C::ContextMenu,
        111 => C::F12,
        113 => C::F15,
        114 => C::Insert,
        115 => C::Home,
        116 => C::PageUp,
        117 => C::Delete,
        118 => C::F4,
        119 => C::End,
        120 => C::F2,
        121 => C::PageDown,
        122 => C::F1,
        123 => C::ArrowLeft,
        124 => C::ArrowRight,
        125 => C::ArrowDown,
        126 => C::ArrowUp,
        other => C::Unknown(u32::from(other)),
    }
}

/// Turns one tap event into a raw key, or `None` if it is not a key event.
///
/// For a modifier, the flag being clear means every key of that kind is up,
/// so it is a release. The flag being set means a press, unless this very key
/// is already down: then its partner (the other shift, say) is still holding
/// the flag and this key was released.
///
/// Caps lock is different: its flag is the lock state, not whether the key is
/// down, and macOS may report only the toggle. So every change of the lock
/// state is one press (`follow_up` supplies its release), and an event that
/// leaves the state as it was is ignored.
fn raw_from_event(
    state: &mut ModifierState,
    event_type: u32,
    keycode: u16,
    flags: u64,
) -> Option<RawKey> {
    match event_type {
        KEY_DOWN => Some(RawKey::Press(code_from_keycode(keycode))),
        KEY_UP => Some(RawKey::Release(code_from_keycode(keycode))),
        FLAGS_CHANGED => {
            let flag = modifier_flag(keycode)?;
            let code = code_from_keycode(keycode);
            if flag == FLAG_CAPS_LOCK {
                let on = flags & flag != 0;
                let changed = state.caps_lock_on != Some(on);
                state.caps_lock_on = Some(on);
                return changed.then_some(RawKey::Press(code));
            }
            if flags & flag != 0 && state.press(keycode) {
                Some(RawKey::Press(code))
            } else {
                state.release(keycode);
                Some(RawKey::Release(code))
            }
        }
        _ => None,
    }
}

/// The raw key to deliver straight after `raw`, if any. A caps-lock press is
/// reported by the OS as a change of lock state with no dependable release,
/// so its release is delivered at once.
fn follow_up(raw: RawKey) -> Option<RawKey> {
    match raw {
        RawKey::Press(KeyCode::CapsLock) => Some(RawKey::Release(KeyCode::CapsLock)),
        _ => None,
    }
}

/// Everything the tap callback needs. It lives on the listener thread's
/// stack for as long as the run loop runs.
struct TapContext<F> {
    deliver: F,
    modifiers: ModifierState,
    /// Set once the tap exists, so the callback can switch it back on.
    tap: Option<CFRetained<CFMachPort>>,
    /// Why the callback stopped the run loop, if it did.
    failure: Option<String>,
}

/// Called by the OS on the listener thread for every tapped event.
///
/// # Safety
/// `user_info` must point to a live `TapContext<F>` that nothing else is
/// using, and `event` must be a valid event. `run` guarantees both.
unsafe extern "C-unwind" fn tap_callback<F: FnMut(RawKey)>(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: `user_info` is the `TapContext<F>` that `run` passed to
    // `tap_create`. It outlives the run loop, and the run loop calls this
    // function on one thread, one event at a time, so the borrow is unique.
    let context = unsafe { &mut *user_info.cast::<TapContext<F>>() };

    match event_type.0 {
        // The OS switches a tap off if it is slow or on some user input.
        // Switch it back on, or capture would stop for good.
        TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT => {
            if let Some(tap) = &context.tap {
                CGEvent::tap_enable(tap, true);
            }
        }
        raw_type => {
            // SAFETY: the OS hands the callback a valid event that stays
            // alive for the length of this call.
            let event_ref = unsafe { event.as_ref() };
            let keycode =
                CGEvent::integer_value_field(Some(event_ref), CGEventField::KeyboardEventKeycode);
            let flags = CGEvent::flags(Some(event_ref)).0;
            // A key code outside u16 cannot name a key: drop it.
            if let Ok(keycode) = u16::try_from(keycode)
                && let Some(raw) = raw_from_event(&mut context.modifiers, raw_type, keycode, flags)
            {
                // A panic must not unwind into the OS frames that called
                // us. Stop the listener and report it instead.
                let delivered = catch_unwind(AssertUnwindSafe(|| {
                    (context.deliver)(raw);
                    if let Some(next) = follow_up(raw) {
                        (context.deliver)(next);
                    }
                }));
                if delivered.is_err() {
                    context.failure = Some("the key consumer panicked".into());
                    if let Some(run_loop) = CFRunLoop::current() {
                        run_loop.stop();
                    }
                }
            }
        }
    }
    // A listen-only tap cannot change events; the same one is handed back.
    event.as_ptr()
}

/// Creates the tap and runs it on the calling thread until it stops.
/// Reports through `started` whether the OS let the tap be created.
pub(super) fn run<F: FnMut(RawKey)>(deliver: F, started: Started) -> Result<(), KeyListenError> {
    let mut context = TapContext {
        deliver,
        modifiers: ModifierState::default(),
        tap: None,
        failure: None,
    };
    let context_ptr: *mut TapContext<F> = &raw mut context;

    // SAFETY: `tap_callback::<F>` matches the callback type and upholds its
    // contract. `context_ptr` points to `context`, which is declared before
    // the tap and so outlives it, and is only used through this pointer from
    // here on.
    let tap = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::HIDEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            KEYBOARD_EVENTS,
            Some(tap_callback::<F>),
            context_ptr.cast(),
        )
    };
    let Some(tap) = tap else {
        return fail(
            &started,
            "macOS refused the key event tap. Grant this app (or the terminal running it) \
             the Accessibility / Input Monitoring permission in System Settings, \
             Privacy & Security, then start it again.",
        );
    };
    let Some(source) = CFMachPort::new_run_loop_source(None, Some(&tap), 0) else {
        return fail(
            &started,
            "could not create a run-loop source for the key event tap",
        );
    };
    let Some(run_loop) = CFRunLoop::current() else {
        return fail(&started, "the listener thread has no run loop");
    };
    // SAFETY: reading a constant that CoreFoundation initialises at load.
    let common_modes = unsafe { kCFRunLoopCommonModes };
    run_loop.add_source(Some(&source), common_modes);

    // SAFETY: the run loop is not running yet, so the callback is not using
    // the context.
    unsafe { (*context_ptr).tap = Some(tap.clone()) };
    CGEvent::tap_enable(&tap, true);

    let _ = started.send(Ok(()));
    CFRunLoop::run();

    // The loop has ended, so the callback will not run again; switching the
    // tap off first makes sure of it before the context is read and dropped.
    CGEvent::tap_enable(&tap, false);
    // SAFETY: no callback can be running: the run loop that calls it has
    // returned on this same thread.
    let failure = unsafe { (*context_ptr).failure.take() };
    Err(KeyListenError::Stopped(failure.unwrap_or_else(|| {
        "the key event tap's run loop ended".into()
    })))
}

/// Reports a start-up failure to `start` and returns it.
fn fail(started: &Started, reason: &str) -> Result<(), KeyListenError> {
    let error = KeyListenError::Start(reason.into());
    let _ = started.send(Err(error.clone()));
    Err(error)
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
