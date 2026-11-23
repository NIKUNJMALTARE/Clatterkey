//! Tests for playing a loaded pack through `Player` (feature 04: AC2 to AC7,
//! AC13, AC17). A fixture pack gives each group its own sample length, so the
//! length of what the fake sink received names the group that played:
//! default 40/45/50 ms, space 70, enter 90, backspace 110, modifier 130,
//! up default 150, up space 170.
//!
//! Signature assumed from the plan: `Player::with_sounds(Arc<dyn Sink>, SoundSet)`.

use std::fs;
use std::sync::Arc;
use std::time::Instant;

use mechanical_key_sounds_lib::audio::{Player, builtin_click};
use mechanical_key_sounds_lib::keys::{KeyCode, KeyKind};
use mechanical_key_sounds_lib::packs::load;
use mechanical_key_sounds_lib::startup::key_consumer;

mod common;
use common::*;

fn player_for(pack: &str) -> (Player, Arc<RecordingSink>) {
    let loaded = load(&fixture_pack(pack)).expect("fixture pack loads");
    let sink = Arc::new(RecordingSink::default());
    (Player::with_sounds(sink.clone(), loaded.sounds), sink)
}

/// The lengths in ms of what played for one event, in order.
fn lengths(sink: &RecordingSink) -> Vec<u64> {
    sink.samples().iter().map(ms).collect()
}

const DEFAULT_MS: [u64; 3] = [40, 45, 50];

#[test]
fn ac2_each_key_plays_from_its_group() {
    let (player, sink) = player_for("full");

    let cases: Vec<(KeyCode, Vec<u64>)> = vec![
        (KeyCode::KeyA, DEFAULT_MS.to_vec()),
        (KeyCode::Digit7, DEFAULT_MS.to_vec()),
        (KeyCode::F5, DEFAULT_MS.to_vec()),
        (KeyCode::ArrowLeft, DEFAULT_MS.to_vec()),
        (KeyCode::Unknown(999), DEFAULT_MS.to_vec()),
        (KeyCode::Space, vec![70]),
        (KeyCode::Enter, vec![90]),
        (KeyCode::NumpadEnter, vec![90]),
        (KeyCode::Backspace, vec![110]),
    ];
    for (code, allowed) in cases {
        player.handle(down(code));
        let got = lengths(&sink);
        assert_eq!(got.len(), 1, "{code}: one sample per Down");
        assert!(allowed.contains(&got[0]), "{code}: played {} ms", got[0]);
        sink.played.lock().unwrap().clear();
    }
}

#[test]
fn ac2_every_modifier_plays_the_modifier_group() {
    let (player, sink) = player_for("full");
    for code in MODIFIERS {
        player.handle(down(code));
    }
    assert_eq!(lengths(&sink), vec![130; MODIFIERS.len()]);
}

#[test]
fn ac3_with_only_default_every_key_plays_from_default() {
    let (player, sink) = player_for("default-only");
    let keys = [
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::ShiftLeft,
        KeyCode::KeyA,
    ];
    for code in keys {
        player.handle(down(code));
    }
    let got = lengths(&sink);
    assert_eq!(got.len(), keys.len());
    for (code, len) in keys.iter().zip(got) {
        assert!(DEFAULT_MS.contains(&len), "{code}: {len} ms");
    }
}

#[test]
fn ac4_300_downs_use_every_default_file_and_never_repeat_one() {
    let (player, sink) = player_for("full");
    for _ in 0..300 {
        player.handle(down(KeyCode::KeyA));
    }
    let got = lengths(&sink);
    assert_eq!(got.len(), 300);
    for len in DEFAULT_MS {
        assert!(got.contains(&len), "file of {len} ms never played");
    }
    for pair in got.windows(2) {
        assert_ne!(pair[0], pair[1], "same file twice in a row");
    }
}

#[test]
fn ac4_a_group_with_one_file_plays_it_every_time() {
    let (player, sink) = player_for("full");
    for _ in 0..300 {
        player.handle(down(KeyCode::Space));
    }
    let samples = sink.samples();
    assert_eq!(samples.len(), 300);
    assert!(samples.iter().all(|s| s.same_buffer(&samples[0])));
    assert_eq!(ms(&samples[0]), 70);
}

#[test]
fn ac5_up_space_and_up_default() {
    let (player, sink) = player_for("full");
    player.handle(up(KeyCode::Space));
    player.handle(up(KeyCode::KeyA));
    assert_eq!(lengths(&sink), vec![170, 150]);
}

#[test]
fn ac5_up_groups_without_their_own_sound_use_up_default() {
    let (player, sink) = player_for("full");
    player.handle(up(KeyCode::Enter));
    player.handle(up(KeyCode::Backspace));
    player.handle(up(KeyCode::ShiftLeft));
    assert_eq!(lengths(&sink), vec![150, 150, 150]);
}

#[test]
fn ac6_up_space_only_plays_nothing_for_a_letter_and_space_for_space() {
    let (player, sink) = player_for("up-space-only");
    player.handle(up(KeyCode::KeyA));
    assert_eq!(sink.count(), 0);
    player.handle(up(KeyCode::Space));
    assert_eq!(lengths(&sink), vec![170]);
}

#[test]
fn ac7_a_pack_with_no_up_section_plays_nothing_on_any_up() {
    let (player, sink) = player_for("no-up");
    for code in [
        KeyCode::KeyA,
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::ShiftLeft,
        KeyCode::Unknown(5),
    ] {
        player.handle(up(code));
    }
    assert_eq!(sink.count(), 0);
}

#[test]
fn down_and_up_both_play_when_the_pack_has_up_sounds() {
    let (player, sink) = player_for("full");
    player.handle(down(KeyCode::Space));
    player.handle(up(KeyCode::Space));
    assert_eq!(lengths(&sink), vec![70, 170]);
}

#[test]
fn feature_03_player_new_still_plays_the_click_on_down_only() {
    let sink = Arc::new(RecordingSink::default());
    let click = builtin_click();
    let player = Player::new(sink.clone(), click.clone());
    player.handle(down(KeyCode::Space));
    player.handle(up(KeyCode::Space));
    player.handle(down(KeyCode::ShiftLeft));
    let played = sink.samples();
    assert_eq!(played.len(), 2);
    assert!(played.iter().all(|s| s.same_buffer(&click)));
}

#[test]
fn ac13_handling_reads_no_file_the_pack_can_be_deleted_after_loading() {
    let tmp = TempDir::new("gone");
    let dir = tmp.path().join("full");
    copy_dir(&fixture_pack("full"), &dir);
    let loaded = load(&dir).unwrap();
    fs::remove_dir_all(&dir).unwrap();

    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_sounds(sink.clone(), loaded.sounds);
    player.handle(down(KeyCode::Space));
    player.handle(up(KeyCode::Space));
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink.count(), 3);
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn ac13_median_handling_time_is_under_1_ms_over_1000_random_events() {
    let (player, sink) = player_for("full");
    let keys = [
        KeyCode::KeyA,
        KeyCode::KeyQ,
        KeyCode::Digit3,
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::ShiftLeft,
        KeyCode::ControlRight,
        KeyCode::F5,
        KeyCode::ArrowUp,
        KeyCode::Unknown(77),
    ];
    let mut state: u32 = 0x1234_5678;
    let mut times = Vec::with_capacity(1000);
    for _ in 0..1000 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let code = keys[state as usize % keys.len()];
        let event = if state & 0x100 == 0 {
            down(code)
        } else {
            up(code)
        };
        let start = Instant::now();
        player.handle(event);
        times.push(start.elapsed());
    }
    times.sort();
    let median = times[times.len() / 2];
    assert!(median.as_micros() < 1000, "median handling time {median:?}");
    assert!(sink.count() > 500);
}

#[test]
fn ac17_handling_key_events_writes_nothing() {
    let loaded = load(&fixture_pack("full")).unwrap();
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();
    let mut consumer = key_consumer(Player::with_sounds(sink.clone(), loaded.sounds), err);

    for code in [
        KeyCode::KeyA,
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::ShiftLeft,
        KeyCode::F5,
        KeyCode::Unknown(999),
    ] {
        consumer(down(code));
        consumer(up(code));
    }

    assert_eq!(written(&buffer), "");
    assert_eq!(
        sink.count(),
        7 + 7,
        "full pack has down and up for every key"
    );
}

#[test]
fn up_kind_is_not_mistaken_for_down() {
    // Guards the KeyKind -> list choice: an Up must never play a down sample.
    let (player, sink) = player_for("full");
    player.handle(up(KeyCode::KeyA));
    assert_eq!(lengths(&sink), vec![150]);
    assert!(!DEFAULT_MS.contains(&lengths(&sink)[0]));
    let _ = KeyKind::Up;
}
