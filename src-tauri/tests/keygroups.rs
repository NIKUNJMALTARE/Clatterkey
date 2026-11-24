//! Tests for `keys::group_of` and `KeyGroup` (feature 04, AC2 and AC3).
//! The plan puts these beside the code in `keys/group.rs`; they are written
//! here, through the public interface, because the file does not exist yet.

use clickclak_lib::keys::{KeyCode, KeyGroup, group_of};

mod common;
use common::MODIFIERS;

#[test]
fn ac2_space_enter_and_backspace_have_their_own_groups() {
    assert_eq!(group_of(KeyCode::Space), KeyGroup::Space);
    assert_eq!(group_of(KeyCode::Enter), KeyGroup::Enter);
    assert_eq!(group_of(KeyCode::NumpadEnter), KeyGroup::Enter);
    assert_eq!(group_of(KeyCode::Backspace), KeyGroup::Backspace);
}

#[test]
fn ac2_every_modifier_in_the_table_is_modifier() {
    for code in MODIFIERS {
        assert_eq!(group_of(code), KeyGroup::Modifier, "{code}");
    }
}

#[test]
fn ac2_every_other_key_is_default_including_unknown() {
    let others = [
        KeyCode::KeyA,
        KeyCode::KeyZ,
        KeyCode::Digit0,
        KeyCode::Digit5,
        KeyCode::Backquote,
        KeyCode::Slash,
        KeyCode::Tab,
        KeyCode::Escape,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::ArrowUp,
        KeyCode::ArrowLeft,
        KeyCode::Home,
        KeyCode::PageDown,
        KeyCode::F1,
        KeyCode::F5,
        KeyCode::F24,
        KeyCode::PrintScreen,
        KeyCode::NumLock,
        KeyCode::Numpad0,
        KeyCode::NumpadAdd,
        KeyCode::NumpadEqual,
        KeyCode::Unknown(0),
        KeyCode::Unknown(999),
        KeyCode::Unknown(u32::MAX),
    ];
    for code in others {
        assert_eq!(group_of(code), KeyGroup::Default, "{code}");
    }
}

#[test]
fn groups_have_five_distinct_indices_from_zero_to_four() {
    assert_eq!(KeyGroup::ALL.len(), 5);
    let mut seen = [false; 5];
    for group in KeyGroup::ALL {
        let i = group.index();
        assert!(i < 5);
        assert!(!seen[i], "index {i} used twice");
        seen[i] = true;
    }
}

#[test]
fn group_names_match_pack_json_and_round_trip() {
    let expected = [
        (KeyGroup::Default, "default"),
        (KeyGroup::Space, "space"),
        (KeyGroup::Enter, "enter"),
        (KeyGroup::Backspace, "backspace"),
        (KeyGroup::Modifier, "modifier"),
    ];
    for (group, name) in expected {
        assert_eq!(group.name(), name);
        assert_eq!(KeyGroup::from_name(name), Some(group));
    }
}

#[test]
fn from_name_is_exact_so_a_typo_is_not_a_group() {
    for bad in ["Space", "SPACE", " space", "space ", "tab", "arrows", ""] {
        assert_eq!(KeyGroup::from_name(bad), None, "{bad:?}");
    }
}
