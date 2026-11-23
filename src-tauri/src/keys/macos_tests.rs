//! Unit tests for the pure mapping functions of the macOS back end.

use super::*;

// CGEventType
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const LEFT_MOUSE_DOWN: u32 = 1;
const MOUSE_MOVED: u32 = 5;
const SCROLL_WHEEL: u32 = 22;

// CGEventFlags
const CAPS: u64 = 0x10000;
const SHIFT: u64 = 0x20000;
const CONTROL: u64 = 0x40000;
const ALT: u64 = 0x80000;
const COMMAND: u64 = 0x100000;
const FN: u64 = 0x800000;

fn raw(state: &mut ModifierState, ty: u32, keycode: u16, flags: u64) -> Option<RawKey> {
    raw_from_event(state, ty, keycode, flags)
}

fn is_press(r: Option<RawKey>, code: KeyCode) -> bool {
    matches!(r, Some(RawKey::Press(c)) if c == code)
}

fn is_release(r: Option<RawKey>, code: KeyCode) -> bool {
    matches!(r, Some(RawKey::Release(c)) if c == code)
}

#[test]
fn ac9_virtual_key_codes_map_to_named_codes() {
    let table: &[(u16, KeyCode)] = &[
        (0, KeyCode::KeyA),
        (18, KeyCode::Digit1),
        (49, KeyCode::Space),
        (36, KeyCode::Enter),
        (51, KeyCode::Backspace),
        (48, KeyCode::Tab),
        (53, KeyCode::Escape),
        (123, KeyCode::ArrowLeft),
        (124, KeyCode::ArrowRight),
        (125, KeyCode::ArrowDown),
        (126, KeyCode::ArrowUp),
        (96, KeyCode::F5),
        (56, KeyCode::ShiftLeft),
        (60, KeyCode::ShiftRight),
        (59, KeyCode::ControlLeft),
        (62, KeyCode::ControlRight),
        (58, KeyCode::AltLeft),
        (61, KeyCode::AltRight),
        (55, KeyCode::MetaLeft),
        (54, KeyCode::MetaRight),
        (57, KeyCode::CapsLock),
        (63, KeyCode::Fn),
    ];
    for (keycode, code) in table {
        assert_eq!(code_from_keycode(*keycode), *code, "virtual key {keycode}");
    }
}

#[test]
fn ac7_unmapped_virtual_key_becomes_unknown_with_its_number() {
    assert_eq!(code_from_keycode(0x7F00), KeyCode::Unknown(0x7F00));
    assert_eq!(code_from_keycode(0xFFFF), KeyCode::Unknown(0xFFFF));
}

#[test]
fn ac9_every_modifier_has_its_own_code() {
    let codes: std::collections::HashSet<KeyCode> = [56u16, 60, 59, 62, 58, 61, 55, 54, 57, 63]
        .iter()
        .map(|k| code_from_keycode(*k))
        .collect();
    assert_eq!(codes.len(), 10);
}

#[test]
fn ac1_key_down_and_key_up_map_directly() {
    let mut s = ModifierState::default();
    assert!(is_press(raw(&mut s, KEY_DOWN, 0, 0), KeyCode::KeyA));
    assert!(is_release(raw(&mut s, KEY_UP, 0, 0), KeyCode::KeyA));
}

#[test]
fn ac2_key_down_events_are_passed_on_for_the_filter_to_deduplicate() {
    let mut s = ModifierState::default();
    assert!(is_press(raw(&mut s, KEY_DOWN, 49, 0), KeyCode::Space));
    assert!(is_press(raw(&mut s, KEY_DOWN, 49, 0), KeyCode::Space));
}

#[test]
fn ac7_unmapped_key_down_is_reported_as_unknown() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, KEY_DOWN, 0x7F00, 0),
        KeyCode::Unknown(0x7F00)
    ));
    assert!(is_release(
        raw(&mut s, KEY_UP, 0x7F00, 0),
        KeyCode::Unknown(0x7F00)
    ));
}

#[test]
fn ac6_mouse_and_wheel_event_types_give_none() {
    let mut s = ModifierState::default();
    assert!(raw(&mut s, LEFT_MOUSE_DOWN, 0, 0).is_none());
    assert!(raw(&mut s, MOUSE_MOVED, 0, 0).is_none());
    assert!(raw(&mut s, SCROLL_WHEEL, 0, 0).is_none());
}

#[test]
fn ac6_other_event_types_give_none_even_with_a_key_code() {
    let mut s = ModifierState::default();
    for ty in [0u32, 2, 3, 4, 6, 7, 8, 13, 22, 23, 9999] {
        assert!(raw(&mut s, ty, 0, 0).is_none(), "event type {ty}");
    }
}

#[test]
fn ac8_shift_press_and_release_via_flags_changed() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 56, SHIFT),
        KeyCode::ShiftLeft
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 56, 0),
        KeyCode::ShiftLeft
    ));
}

#[test]
fn ac8_second_shift_while_first_is_held_is_a_press() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 56, SHIFT),
        KeyCode::ShiftLeft
    ));
    // The shift flag is still set because left shift is held.
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 60, SHIFT),
        KeyCode::ShiftRight
    ));
}

#[test]
fn ac8_left_up_then_right_up() {
    let mut s = ModifierState::default();
    raw(&mut s, FLAGS_CHANGED, 56, SHIFT);
    raw(&mut s, FLAGS_CHANGED, 60, SHIFT);
    // Left released while right is still held: flag stays set.
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 56, SHIFT),
        KeyCode::ShiftLeft
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 60, 0),
        KeyCode::ShiftRight
    ));
}

#[test]
fn ac8_right_up_then_left_up() {
    let mut s = ModifierState::default();
    raw(&mut s, FLAGS_CHANGED, 56, SHIFT);
    raw(&mut s, FLAGS_CHANGED, 60, SHIFT);
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 60, SHIFT),
        KeyCode::ShiftRight
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 56, 0),
        KeyCode::ShiftLeft
    ));
}

#[test]
fn ac8_right_first_then_left_in_every_order() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 60, SHIFT),
        KeyCode::ShiftRight
    ));
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 56, SHIFT),
        KeyCode::ShiftLeft
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 60, SHIFT),
        KeyCode::ShiftRight
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 56, 0),
        KeyCode::ShiftLeft
    ));
}

#[test]
fn ac9_each_modifier_press_and_release_maps_to_its_code() {
    let cases: &[(u16, u64, KeyCode)] = &[
        (56, SHIFT, KeyCode::ShiftLeft),
        (60, SHIFT, KeyCode::ShiftRight),
        (59, CONTROL, KeyCode::ControlLeft),
        (62, CONTROL, KeyCode::ControlRight),
        (58, ALT, KeyCode::AltLeft),
        (61, ALT, KeyCode::AltRight),
        (55, COMMAND, KeyCode::MetaLeft),
        (54, COMMAND, KeyCode::MetaRight),
        (63, FN, KeyCode::Fn),
    ];
    for (keycode, flag, code) in cases {
        let mut s = ModifierState::default();
        assert!(
            is_press(raw(&mut s, FLAGS_CHANGED, *keycode, *flag), *code),
            "press {code}"
        );
        assert!(
            is_release(raw(&mut s, FLAGS_CHANGED, *keycode, 0), *code),
            "release {code}"
        );
    }
}

#[test]
fn ac9_caps_lock_gives_a_press_whenever_the_flag_changes_and_never_a_release() {
    let mut s = ModifierState::default();
    // The first event always counts as a change, whether the flag is set or clear.
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 57, CAPS),
        KeyCode::CapsLock
    ));
    assert!(raw(&mut s, FLAGS_CHANGED, 57, CAPS).is_none());
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 57, 0),
        KeyCode::CapsLock
    ));
    assert!(raw(&mut s, FLAGS_CHANGED, 57, 0).is_none());
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 57, CAPS),
        KeyCode::CapsLock
    ));
}

#[test]
fn ac9_caps_lock_first_event_with_flag_clear_is_a_press() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 57, 0),
        KeyCode::CapsLock
    ));
}

#[test]
fn ac9_caps_lock_ignores_other_flags_in_the_event() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 57, CAPS | SHIFT),
        KeyCode::CapsLock
    ));
    assert!(raw(&mut s, FLAGS_CHANGED, 57, CAPS | COMMAND).is_none());
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 57, SHIFT),
        KeyCode::CapsLock
    ));
}

#[test]
fn ac1_follow_up_releases_caps_lock_straight_after_its_press() {
    assert!(is_release(
        follow_up(RawKey::Press(KeyCode::CapsLock)),
        KeyCode::CapsLock
    ));
}

#[test]
fn ac1_follow_up_is_none_for_every_other_key() {
    assert!(follow_up(RawKey::Press(KeyCode::KeyA)).is_none());
    assert!(follow_up(RawKey::Press(KeyCode::ShiftLeft)).is_none());
    assert!(follow_up(RawKey::Release(KeyCode::CapsLock)).is_none());
    assert!(follow_up(RawKey::Release(KeyCode::KeyA)).is_none());
}

#[test]
fn ac9_fn_key_press_and_release() {
    let mut s = ModifierState::default();
    assert!(is_press(raw(&mut s, FLAGS_CHANGED, 63, FN), KeyCode::Fn));
    assert!(is_release(raw(&mut s, FLAGS_CHANGED, 63, 0), KeyCode::Fn));
}

#[test]
fn ac8_different_modifiers_do_not_disturb_each_other() {
    let mut s = ModifierState::default();
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 56, SHIFT),
        KeyCode::ShiftLeft
    ));
    assert!(is_press(
        raw(&mut s, FLAGS_CHANGED, 55, SHIFT | COMMAND),
        KeyCode::MetaLeft
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 56, COMMAND),
        KeyCode::ShiftLeft
    ));
    assert!(is_release(
        raw(&mut s, FLAGS_CHANGED, 55, 0),
        KeyCode::MetaLeft
    ));
}

#[test]
fn flags_changed_for_a_non_modifier_key_code_is_ignored() {
    let mut s = ModifierState::default();
    assert!(raw(&mut s, FLAGS_CHANGED, 0, SHIFT).is_none());
    assert!(raw(&mut s, FLAGS_CHANGED, 49, 0).is_none());
    assert!(raw(&mut s, FLAGS_CHANGED, 0x7F00, SHIFT).is_none());
}
