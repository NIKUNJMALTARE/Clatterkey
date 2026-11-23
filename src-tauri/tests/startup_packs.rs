//! Tests for choosing the sounds at start-up (feature 04: AC14 to AC16, as
//! changed by feature 05: the pack is now `packs/<pack_id>`, with fallback to
//! `brown` and then the built-in click) and for `run_sounds`, with a fake
//! listener and a fake sink.
//!
//! Signatures assumed from the plan:
//!
//! ```ignore
//! pub fn choose_sounds(packs: Option<&Path>, pack_id: &str, err: &ErrorOut) -> SoundSet;
//! pub fn run_sounds(
//!     start: impl FnOnce(Consumer) -> Result<Wait, KeyListenError>,
//!     sink: Arc<dyn Sink>,
//!     sounds: SoundSet,
//!     err: ErrorOut,
//! );
//! ```
//!
//! Not testable here: that `lib.rs` reads settings and calls `choose_sounds`
//! before `keys::start` (feature 05 AC13). That is wiring on the start-up
//! thread and needs the running app; `run_sounds` taking a finished
//! `SoundSet` is what is tested.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use mechanical_key_sounds_lib::audio::{SoundSet, builtin_click};
use mechanical_key_sounds_lib::keys::{KeyCode, KeyGroup, KeyKind, KeyListenError};
use mechanical_key_sounds_lib::startup::{Wait, choose_sounds, run_clicks, run_sounds};

mod common;
use common::*;

fn is_builtin_click(sounds: &SoundSet) -> bool {
    let click = builtin_click();
    for group in KeyGroup::ALL {
        let Some(s) = sounds.pick(KeyKind::Down, group) else {
            return false;
        };
        if s.samples() != click.samples() {
            return false;
        }
        if sounds.pick(KeyKind::Up, group).is_some() {
            return false;
        }
    }
    true
}

fn one_sound_line(text: &str) {
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert!(text.starts_with("sound pack: "), "got: {text:?}");
    assert!(
        text.trim_end().ends_with("; using the built-in click"),
        "got: {text:?}"
    );
    assert!(text.ends_with('\n'));
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A packs folder holding the `full` fixture as `full` and as `brown`, so a
/// failing chosen pack falls back to a working brown.
fn packs_with_brown() -> TempDir {
    let tmp = TempDir::new("packs-with-brown");
    copy_dir(&fixture_pack("full"), &tmp.path().join("full"));
    copy_dir(&fixture_pack("full"), &tmp.path().join("brown"));
    tmp
}

fn is_brown_fixture(sounds: &SoundSet) -> bool {
    ms(sounds.pick(KeyKind::Down, KeyGroup::Space).unwrap()) == 70
}

#[test]
fn ac14_with_no_packs_folder_the_builtin_click_plays_after_one_line() {
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(None, "brown", &err);
    one_sound_line(&written(&buffer));
    assert!(is_builtin_click(&sounds));
}

#[test]
fn ac14_run_sounds_with_the_builtin_click_plays_on_down_and_prints_nothing() {
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();
    let sounds = SoundSet::single(builtin_click());

    run_sounds(
        |mut consumer| {
            Ok(Box::new(move || {
                consumer(down(KeyCode::KeyA));
                consumer(up(KeyCode::KeyA));
                Ok(())
            }) as Wait)
        },
        sink.clone(),
        sounds,
        err,
    );

    assert_eq!(written(&buffer), "");
    let played = sink.samples();
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].samples(), builtin_click().samples());
}

#[test]
fn ac15_a_valid_pack_is_chosen_and_plays_by_group() {
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(&fixtures().join("packs")), "full", &err);
    assert_eq!(written(&buffer), "", "a valid pack prints nothing");
    assert!(!is_builtin_click(&sounds));

    let sink = Arc::new(RecordingSink::default());
    run_sounds(
        |mut consumer| {
            Ok(Box::new(move || {
                consumer(down(KeyCode::Space));
                consumer(down(KeyCode::Enter));
                consumer(up(KeyCode::Space));
                Ok(())
            }) as Wait)
        },
        sink.clone(),
        sounds,
        err,
    );

    let got: Vec<u64> = sink.samples().iter().map(ms).collect();
    assert_eq!(got, vec![70, 90, 170]);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac15_the_pack_is_ready_before_the_listener_is_started() {
    // run_sounds is handed a finished SoundSet, so the pack is fully loaded
    // before `start` runs; the first event the fake listener sends already
    // plays a pack sample.
    let (_, err) = fake_err();
    let sounds = choose_sounds(Some(&fixtures().join("packs")), "full", &err);
    let sink = Arc::new(RecordingSink::default());
    run_sounds(
        |mut consumer| {
            consumer(down(KeyCode::Backspace));
            Ok(Box::new(|| Ok(())) as Wait)
        },
        sink.clone(),
        sounds,
        err,
    );
    assert_eq!(sink.samples().iter().map(ms).collect::<Vec<_>>(), vec![110]);
}

#[test]
fn ac16_a_missing_pack_falls_back_to_brown_with_exactly_one_line() {
    let tmp = packs_with_brown();
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "nope", &err);
    assert!(is_brown_fixture(&sounds));
    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "{text:?}");
    assert!(text.starts_with("sound pack: "), "{text}");
    assert!(text.contains("nope"), "{text}");
    assert!(text.trim_end().ends_with("; using \"brown\""), "{text}");
}

#[test]
fn ac16_an_invalid_pack_falls_back_to_brown_with_exactly_one_line() {
    let tmp = packs_with_brown();
    fs::create_dir(tmp.path().join("bad")).unwrap();
    fs::write(tmp.path().join("bad").join("pack.json"), "{ not json").unwrap();
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "bad", &err);
    assert!(is_brown_fixture(&sounds));
    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "{text:?}");
    assert!(text.starts_with("sound pack: "), "{text}");
    assert!(text.contains("pack \"bad\""), "{text}");
    assert!(text.trim_end().ends_with("; using \"brown\""), "{text}");
}

#[test]
fn ac16_a_pack_with_one_bad_file_falls_back_not_to_a_partial_pack() {
    let tmp = packs_with_brown();
    let pack = tmp.path().join("partial");
    fs::create_dir(&pack).unwrap();
    fs::write(
        pack.join("pack.json"),
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"],"space":["gone.wav"]}}"#,
    )
    .unwrap();
    fs::copy(fixture_audio("tone-mono-44100.wav"), pack.join("a.wav")).unwrap();
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "partial", &err);
    assert!(is_brown_fixture(&sounds));
    assert_eq!(written(&buffer).lines().count(), 1);
}

#[test]
fn ac16_with_no_working_brown_the_builtin_click_plays_after_two_lines() {
    let pack = TestPack::new("bad", "{ not json");
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(pack.tmp.path()), "bad", &err);
    assert!(is_builtin_click(&sounds));
    let text = written(&buffer);
    assert_eq!(text.lines().count(), 2, "{text:?}");
    assert!(
        text.trim_end().ends_with("; using the built-in click"),
        "{text}"
    );
}

#[test]
fn ac16_the_error_line_has_no_key_codes() {
    let pack = TestPack::new("clean", &json_default(&["gone.wav"]));
    let (buffer, err) = fake_err();
    let _ = choose_sounds(Some(pack.tmp.path()), "clean", &err);
    let text = written(&buffer);
    assert!(
        !text.contains("KeyA") && !text.contains("Unknown("),
        "{text}"
    );
}

#[test]
fn ac17_playing_a_pack_writes_nothing_to_the_error_output() {
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(&fixtures().join("packs")), "full", &err);
    let sink = Arc::new(RecordingSink::default());
    run_sounds(
        |mut consumer| {
            Ok(Box::new(move || {
                for code in [KeyCode::KeyA, KeyCode::Space, KeyCode::Unknown(9)] {
                    consumer(down(code));
                    consumer(up(code));
                }
                Ok(())
            }) as Wait)
        },
        sink,
        sounds,
        err,
    );
    assert_eq!(written(&buffer), "");
}

#[test]
fn run_sounds_reports_a_listener_that_refuses_to_start_like_run_clicks() {
    let (buffer, err) = fake_err();
    let sink = Arc::new(RecordingSink::default());
    run_sounds(
        |_c| Err(KeyListenError::Start("refused".into())),
        sink.clone(),
        SoundSet::single(builtin_click()),
        err,
    );
    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1);
    assert!(text.starts_with("key listener: ") && text.contains("refused"));
    assert_eq!(sink.count(), 0);
}

#[test]
fn feature_03_run_clicks_still_exists_with_its_signature() {
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();
    run_clicks(
        |mut consumer| {
            Ok(Box::new(move || {
                consumer(down(KeyCode::KeyA));
                Ok(())
            }) as Wait)
        },
        sink.clone(),
        err,
    );
    assert_eq!(written(&buffer), "");
    assert_eq!(sink.count(), 1);
}
