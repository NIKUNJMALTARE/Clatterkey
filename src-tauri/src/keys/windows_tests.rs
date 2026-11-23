//! Unit tests for the pure mapping functions of the Windows back end.
//! Compiled only on Windows.

use super::*;
use rdev::{Button, EventType, Key};

#[test]
fn ac9_rdev_keys_map_to_named_codes() {
    let table: &[(Key, KeyCode)] = &[
        (Key::KeyA, KeyCode::KeyA),
        (Key::Num1, KeyCode::Digit1),
        (Key::Space, KeyCode::Space),
        (Key::Return, KeyCode::Enter),
        (Key::Backspace, KeyCode::Backspace),
        (Key::Tab, KeyCode::Tab),
        (Key::Escape, KeyCode::Escape),
        (Key::UpArrow, KeyCode::ArrowUp),
        (Key::DownArrow, KeyCode::ArrowDown),
        (Key::LeftArrow, KeyCode::ArrowLeft),
        (Key::RightArrow, KeyCode::ArrowRight),
        (Key::F5, KeyCode::F5),
        (Key::ShiftLeft, KeyCode::ShiftLeft),
        (Key::ShiftRight, KeyCode::ShiftRight),
        (Key::ControlLeft, KeyCode::ControlLeft),
        (Key::ControlRight, KeyCode::ControlRight),
        (Key::Alt, KeyCode::AltLeft),
        (Key::AltGr, KeyCode::AltRight),
        (Key::MetaLeft, KeyCode::MetaLeft),
        (Key::MetaRight, KeyCode::MetaRight),
        (Key::CapsLock, KeyCode::CapsLock),
        (Key::Kp1, KeyCode::Numpad1),
    ];
    for (key, code) in table {
        assert_eq!(code_from_rdev(*key), *code, "{key:?}");
    }
}

#[test]
fn ac8_left_and_right_modifiers_are_distinct() {
    assert_ne!(
        code_from_rdev(Key::ShiftLeft),
        code_from_rdev(Key::ShiftRight)
    );
    assert_ne!(
        code_from_rdev(Key::ControlLeft),
        code_from_rdev(Key::ControlRight)
    );
    assert_ne!(code_from_rdev(Key::Alt), code_from_rdev(Key::AltGr));
    assert_ne!(
        code_from_rdev(Key::MetaLeft),
        code_from_rdev(Key::MetaRight)
    );
}

#[test]
fn ac7_unknown_rdev_key_keeps_its_number() {
    assert_eq!(code_from_rdev(Key::Unknown(1234)), KeyCode::Unknown(1234));
}

#[test]
fn ac1_key_press_and_release_events_map_to_raw_keys() {
    assert!(matches!(
        raw_from_rdev(&EventType::KeyPress(Key::KeyA)),
        Some(RawKey::Press(KeyCode::KeyA))
    ));
    assert!(matches!(
        raw_from_rdev(&EventType::KeyRelease(Key::KeyA)),
        Some(RawKey::Release(KeyCode::KeyA))
    ));
}

#[test]
fn ac7_unknown_key_events_are_reported() {
    assert!(matches!(
        raw_from_rdev(&EventType::KeyPress(Key::Unknown(99))),
        Some(RawKey::Press(KeyCode::Unknown(99)))
    ));
    assert!(matches!(
        raw_from_rdev(&EventType::KeyRelease(Key::Unknown(99))),
        Some(RawKey::Release(KeyCode::Unknown(99)))
    ));
}

#[test]
fn ac6_mouse_button_move_and_wheel_events_give_none() {
    assert!(raw_from_rdev(&EventType::ButtonPress(Button::Left)).is_none());
    assert!(raw_from_rdev(&EventType::ButtonRelease(Button::Left)).is_none());
    assert!(raw_from_rdev(&EventType::MouseMove { x: 1.0, y: 2.0 }).is_none());
    assert!(
        raw_from_rdev(&EventType::Wheel {
            delta_x: 0,
            delta_y: 1
        })
        .is_none()
    );
}
