//! Tests for the `keys` module through its public interface: key codes, the
//! auto-repeat filter, events and the listener errors. No real key capture.

use std::collections::HashSet;

use clatterkey_lib::keys::{KeyCode, KeyEvent, KeyKind, KeyListenError, RawKey, RepeatFilter};

/// Renders an event as `down KeyA`. Destructures `KeyEvent` exhaustively, so
/// adding a field to it fails to compile (AC10).
fn line(event: KeyEvent) -> String {
    let KeyEvent { code, kind } = event;
    format!("{kind} {code}")
}

/// Feeds raw keys through a fresh filter and returns the delivered events as lines.
fn run(raws: &[RawKey]) -> Vec<String> {
    let mut filter = RepeatFilter::new();
    raws.iter()
        .filter_map(|raw| filter.accept(*raw).map(line))
        .collect()
}

use KeyCode::*;
use RawKey::{Press, Release};

#[test]
fn ac1_press_then_release_gives_one_down_then_one_up() {
    assert_eq!(run(&[Press(KeyA), Release(KeyA)]), ["down KeyA", "up KeyA"]);
}

#[test]
fn ac2_held_key_gives_one_down_and_one_up() {
    let mut raws = vec![Press(KeyA); 20];
    raws.push(Release(KeyA));
    assert_eq!(run(&raws), ["down KeyA", "up KeyA"]);
}

#[test]
fn ac2_two_held_keys_repeating_at_once_give_one_down_each() {
    let raws = [
        Press(KeyA),
        Press(KeyB),
        Press(KeyA),
        Press(KeyB),
        Press(KeyA),
        Press(KeyB),
        Release(KeyA),
        Release(KeyB),
    ];
    assert_eq!(run(&raws), ["down KeyA", "down KeyB", "up KeyA", "up KeyB"]);
}

#[test]
fn ac2_releases_in_opposite_order_to_presses() {
    let raws = [
        Press(KeyA),
        Press(KeyB),
        Press(KeyB),
        Release(KeyB),
        Release(KeyA),
    ];
    assert_eq!(run(&raws), ["down KeyA", "down KeyB", "up KeyB", "up KeyA"]);
}

#[test]
fn ac3_tapping_b_while_a_is_held_gives_two_b_pairs_and_one_a_down() {
    let raws = [
        Press(KeyA),
        Press(KeyB),
        Release(KeyB),
        Press(KeyA), // auto-repeat of A
        Press(KeyB),
        Press(KeyA), // auto-repeat of A
        Release(KeyB),
        Release(KeyA),
    ];
    assert_eq!(
        run(&raws),
        [
            "down KeyA",
            "down KeyB",
            "up KeyB",
            "down KeyB",
            "up KeyB",
            "up KeyA"
        ]
    );
}

#[test]
fn ac4_press_after_release_gives_a_new_down() {
    let raws = [Press(KeyA), Release(KeyA), Press(KeyA), Release(KeyA)];
    assert_eq!(run(&raws), ["down KeyA", "up KeyA", "down KeyA", "up KeyA"]);
}

#[test]
fn ac5_release_without_press_is_delivered_and_next_press_gives_down() {
    assert_eq!(run(&[Release(KeyA)]), ["up KeyA"]);
    assert_eq!(
        run(&[Release(KeyA), Press(KeyA), Press(KeyA), Release(KeyA)]),
        ["up KeyA", "down KeyA", "up KeyA"]
    );
}

#[test]
fn ac5_repeated_release_is_delivered_each_time() {
    assert_eq!(run(&[Release(KeyA), Release(KeyA)]), ["up KeyA", "up KeyA"]);
}

#[test]
fn ac7_unknown_code_is_reported_and_displays_its_number() {
    assert_eq!(
        run(&[Press(Unknown(1234)), Release(Unknown(1234))]),
        ["down Unknown(1234)", "up Unknown(1234)"]
    );
    assert_eq!(Unknown(0).to_string(), "Unknown(0)");
}

#[test]
fn ac7_unknown_code_is_repeat_filtered() {
    let raws = [
        Press(Unknown(7)),
        Press(Unknown(7)),
        Press(Unknown(7)),
        Release(Unknown(7)),
    ];
    assert_eq!(run(&raws), ["down Unknown(7)", "up Unknown(7)"]);
}

#[test]
fn ac7_unknown_codes_with_different_numbers_are_independent() {
    assert_ne!(Unknown(1), Unknown(2));
    assert_eq!(
        run(&[
            Press(Unknown(1)),
            Press(Unknown(2)),
            Release(Unknown(1)),
            Release(Unknown(2))
        ]),
        [
            "down Unknown(1)",
            "down Unknown(2)",
            "up Unknown(1)",
            "up Unknown(2)"
        ]
    );
}

#[test]
fn ac8_left_and_right_shift_are_different_codes() {
    assert_ne!(ShiftLeft, ShiftRight);
    assert_ne!(ShiftLeft.to_string(), ShiftRight.to_string());
}

#[test]
fn ac8_left_and_right_shift_have_independent_state() {
    let raws = [
        Press(ShiftLeft),
        Press(ShiftRight),
        Press(ShiftLeft), // repeat
        Release(ShiftLeft),
        Release(ShiftRight),
    ];
    assert_eq!(
        run(&raws),
        [
            "down ShiftLeft",
            "down ShiftRight",
            "up ShiftLeft",
            "up ShiftRight"
        ]
    );
}

#[test]
fn ac9_display_names_follow_the_documented_names() {
    let expected: &[(KeyCode, &str)] = &[
        (KeyA, "KeyA"),
        (KeyZ, "KeyZ"),
        (Digit0, "Digit0"),
        (Digit1, "Digit1"),
        (Digit9, "Digit9"),
        (Space, "Space"),
        (Enter, "Enter"),
        (Backspace, "Backspace"),
        (Tab, "Tab"),
        (Escape, "Escape"),
        (ArrowUp, "ArrowUp"),
        (ArrowDown, "ArrowDown"),
        (ArrowLeft, "ArrowLeft"),
        (ArrowRight, "ArrowRight"),
        (F1, "F1"),
        (F5, "F5"),
        (F12, "F12"),
        (F24, "F24"),
        (ShiftLeft, "ShiftLeft"),
        (ShiftRight, "ShiftRight"),
        (ControlLeft, "ControlLeft"),
        (ControlRight, "ControlRight"),
        (AltLeft, "AltLeft"),
        (AltRight, "AltRight"),
        (MetaLeft, "MetaLeft"),
        (MetaRight, "MetaRight"),
        (CapsLock, "CapsLock"),
        (Fn, "Fn"),
        (Numpad0, "Numpad0"),
        (Numpad1, "Numpad1"),
        (Numpad9, "Numpad9"),
    ];
    for (code, name) in expected {
        assert_eq!(code.to_string(), *name);
    }
}

#[test]
fn ac9_the_listed_keys_are_all_distinct_codes_and_names() {
    let codes = [
        KeyA,
        Digit1,
        Space,
        Enter,
        Backspace,
        Tab,
        Escape,
        ArrowUp,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        F5,
        ShiftLeft,
        ShiftRight,
        ControlLeft,
        ControlRight,
        AltLeft,
        AltRight,
        MetaLeft,
        MetaRight,
        CapsLock,
        Fn,
        Numpad1,
    ];
    let set: HashSet<KeyCode> = codes.iter().copied().collect();
    let names: HashSet<String> = codes.iter().map(|c| c.to_string()).collect();
    assert_eq!(set.len(), codes.len());
    assert_eq!(names.len(), codes.len());
}

#[test]
fn ac9_key_codes_are_copy_eq_and_hashable() {
    let a = KeyA;
    let b = a; // Copy
    assert_eq!(a, b);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

#[test]
fn ac1_kind_displays_as_down_and_up() {
    assert_eq!(KeyKind::Down.to_string(), "down");
    assert_eq!(KeyKind::Up.to_string(), "up");
}

#[test]
fn ac10_key_event_is_only_a_code_and_a_kind() {
    let event = KeyEvent {
        code: KeyA,
        kind: KeyKind::Down,
    };
    let KeyEvent { code, kind } = event;
    assert_eq!(code, KeyA);
    assert_eq!(kind.to_string(), "down");
}

#[test]
fn ac10_filter_output_is_a_down_or_up_event_with_the_same_code() {
    let mut filter = RepeatFilter::new();
    let down = filter.accept(Press(Digit1)).expect("down");
    assert_eq!(
        (down.code, down.kind.to_string()),
        (Digit1, "down".to_string())
    );
    let up = filter.accept(Release(Digit1)).expect("up");
    assert_eq!((up.code, up.kind.to_string()), (Digit1, "up".to_string()));
}

#[test]
fn error_displays() {
    assert_eq!(
        KeyListenError::Unsupported.to_string(),
        "key capture is not supported on this platform"
    );
    assert!(
        KeyListenError::Start("boom".into())
            .to_string()
            .contains("boom")
    );
    assert!(
        KeyListenError::Stopped("gone".into())
            .to_string()
            .contains("gone")
    );
    let _: &dyn std::error::Error = &KeyListenError::Unsupported;
}

/// AC11. Compiled only where key capture is compiled out. Never call `start`
/// on macOS or Windows from a test: it would install a real key tap.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn ac11_start_returns_unsupported_on_other_platforms() {
    let result = clatterkey_lib::keys::start(|_event| {});
    assert!(matches!(result, Err(KeyListenError::Unsupported)));
}
