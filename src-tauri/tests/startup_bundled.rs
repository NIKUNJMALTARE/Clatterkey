//! Tests for choosing the pack from settings at start-up (feature 05: AC6,
//! AC7, AC10, AC11, AC13, AC14), against the committed bundled packs and
//! temporary pack folders.
//!
//! Interfaces assumed from the plan:
//!
//! ```ignore
//! // startup
//! pub fn is_pack_id(id: &str) -> bool;
//! pub fn choose_sounds(packs: Option<&Path>, pack_id: &str, err: &ErrorOut) -> SoundSet;
//! // settings
//! pub fn load_or_create(path: &Path, err: &ErrorOut) -> Settings;
//! ```
//!
//! A `SoundSet` picks at random, so "this pack plays" is shown by sampling
//! many picks and checking each one is a sample of the expected pack.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clatterkey_lib::audio::{SoundSet, builtin_click};
use clatterkey_lib::keys::{KeyCode, KeyGroup, KeyKind};
use clatterkey_lib::packs;
use clatterkey_lib::settings;
use clatterkey_lib::startup::{Wait, choose_sounds, is_pack_id, run_sounds};

mod common;
use common::*;

fn bundled() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("packs")
}

/// Every distinct buffer a set can play, found by sampling.
fn played_by(sounds: &SoundSet) -> HashSet<Vec<u32>> {
    let mut out = HashSet::new();
    for kind in [KeyKind::Down, KeyKind::Up] {
        for group in KeyGroup::ALL {
            for _ in 0..200 {
                if let Some(s) = sounds.pick(kind, group) {
                    out.insert(s.samples().iter().map(|v| v.to_bits()).collect());
                }
            }
        }
    }
    out
}

fn is_pack(sounds: &SoundSet, id: &str) -> bool {
    let reference = packs::load(&bundled().join(id)).unwrap();
    let want = played_by(&reference.sounds);
    let got = played_by(sounds);
    !got.is_empty() && got == want
}

fn is_builtin_click(sounds: &SoundSet) -> bool {
    let click = builtin_click();
    KeyGroup::ALL.iter().all(|g| {
        sounds
            .pick(KeyKind::Down, *g)
            .is_some_and(|s| s.samples() == click.samples())
            && sounds.pick(KeyKind::Up, *g).is_none()
    })
}

/// A pack folder `packs/<id>` whose only sound is `ms` long.
fn make_pack(packs: &Path, id: &str, ms: u32) {
    let dir = packs.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("pack.json"), json_default(&["a.wav"])).unwrap();
    fs::write(dir.join("a.wav"), wav_bytes(44_100, ms, 8000)).unwrap();
}

fn make_broken_pack(packs: &Path, id: &str) {
    let dir = packs.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("pack.json"), "{ not json").unwrap();
}

fn default_ms(sounds: &SoundSet) -> u64 {
    ms(sounds.pick(KeyKind::Down, KeyGroup::Default).unwrap())
}

// AC10: pack ids

#[test]
fn ac10_is_pack_id_accepts_plain_names_only() {
    for ok in ["blue", "brown", "red", "typewriter", "my-pack_2", "A1"] {
        assert!(is_pack_id(ok), "{ok:?} should be valid");
    }
    for bad in [
        "", ".", "..", "../brown", "brown/..", "/etc", "C:brown", "a/b", "a\\b", " brown",
        "brown ", "bro wn", "brown.", "c:", "\u{e9}", "a\0b",
    ] {
        assert!(!is_pack_id(bad), "{bad:?} should be rejected");
    }
}

#[test]
fn ac10_unknown_ids_play_brown_with_one_line_and_never_leave_the_packs_folder() {
    let tmp = TempDir::new("unknown");
    let packs_dir = tmp.path().join("packs");
    make_pack(&packs_dir, "brown", 50);
    // What "../brown" would reach if the id were joined unchecked.
    make_pack(tmp.path(), "brown", 77);
    make_pack(tmp.path(), "etc", 88);

    let abs = tmp.path().to_string_lossy().into_owned();
    let ids = [
        "nope".to_string(),
        String::new(),
        "../brown".to_string(),
        "brown/..".to_string(),
        "/etc".to_string(),
        "C:brown".to_string(),
        abs,
        " brown".to_string(),
        "brown ".to_string(),
    ];
    for id in ids {
        let (buffer, err) = fake_err();
        let sounds = choose_sounds(Some(&packs_dir), &id, &err);
        assert_eq!(default_ms(&sounds), 50, "id {id:?} must play packs/brown");
        assert_eq!(
            written(&buffer),
            format!("sound pack: unknown pack \"{id}\"; using \"brown\"\n"),
            "id {id:?}"
        );
    }
}

#[test]
fn ac10_a_valid_looking_id_with_no_folder_is_unknown() {
    let tmp = TempDir::new("nofolder");
    make_pack(tmp.path(), "brown", 50);
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "nope", &err);
    assert_eq!(default_ms(&sounds), 50);
    assert_eq!(
        written(&buffer),
        "sound pack: unknown pack \"nope\"; using \"brown\"\n"
    );
}

#[test]
fn ac10_the_settings_file_is_unchanged_by_a_bad_pack_id() {
    let tmp = TempDir::new("keepfile");
    let path = tmp.path().join("settings.json");
    let text = r#"{"pack_id":"nope"}"#;
    fs::write(&path, text).unwrap();
    let (_, err) = fake_err();
    let s = settings::load_or_create(&path, &err);
    let _ = choose_sounds(Some(&bundled()), &s.pack_id, &err);
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
}

// AC6, AC7

#[test]
fn ac6_default_settings_play_brown_from_the_bundled_packs() {
    let tmp = TempDir::new("defaults");
    let path = tmp.path().join("settings.json");
    let (buffer, err) = fake_err();
    let s = settings::load_or_create(&path, &err);
    let sounds = choose_sounds(Some(&bundled()), &s.pack_id, &err);
    assert_eq!(written(&buffer), "");
    assert!(is_pack(&sounds, "brown"));
}

#[test]
fn ac7_each_bundled_id_plays_that_pack_and_prints_nothing() {
    for id in ["blue", "brown", "red", "typewriter"] {
        let (buffer, err) = fake_err();
        let sounds = choose_sounds(Some(&bundled()), id, &err);
        assert_eq!(written(&buffer), "", "pack {id}");
        assert!(is_pack(&sounds, id), "pack {id} should play");
    }
}

#[test]
fn ac7_a_pack_id_read_from_the_settings_file_is_the_pack_that_plays() {
    let tmp = TempDir::new("fromfile");
    let path = tmp.path().join("settings.json");
    fs::write(&path, r#"{"pack_id":"typewriter"}"#).unwrap();
    let (buffer, err) = fake_err();
    let s = settings::load_or_create(&path, &err);
    let sounds = choose_sounds(Some(&bundled()), &s.pack_id, &err);
    assert!(is_pack(&sounds, "typewriter"));
    assert!(
        written(&buffer)
            .lines()
            .all(|l| l.starts_with("settings: "))
    );
}

// AC11

fn sound_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines.iter().all(|l| l.starts_with("sound pack: ")),
        "{text:?}"
    );
    lines
}

#[test]
fn ac11_a_chosen_pack_that_fails_falls_back_to_brown_with_one_line() {
    let tmp = TempDir::new("fallback");
    make_broken_pack(tmp.path(), "red");
    make_pack(tmp.path(), "brown", 50);
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "red", &err);
    assert_eq!(default_ms(&sounds), 50);
    let text = written(&buffer);
    let lines = sound_lines(&text);
    assert_eq!(lines.len(), 1, "{text:?}");
    assert!(lines[0].contains("red"), "{text}");
    assert!(lines[0].ends_with("; using \"brown\""), "{text}");
    assert!(text.ends_with('\n'));
}

#[test]
fn ac11_when_brown_fails_too_the_builtin_click_plays_after_exactly_two_lines() {
    let tmp = TempDir::new("both");
    make_broken_pack(tmp.path(), "red");
    make_broken_pack(tmp.path(), "brown");
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "red", &err);
    assert!(is_builtin_click(&sounds));
    let text = written(&buffer);
    let lines = sound_lines(&text);
    assert_eq!(lines.len(), 2, "{text:?}");
    assert!(lines[0].ends_with("; using \"brown\""), "{text}");
    assert!(lines[1].ends_with("; using the built-in click"), "{text}");
}

#[test]
fn ac11_an_unknown_id_and_a_broken_brown_also_gives_two_lines() {
    let tmp = TempDir::new("unknown-broken");
    make_broken_pack(tmp.path(), "brown");
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "nope", &err);
    assert!(is_builtin_click(&sounds));
    let text = written(&buffer);
    let lines = sound_lines(&text);
    assert_eq!(lines.len(), 2, "{text:?}");
    assert_eq!(
        lines[0],
        "sound pack: unknown pack \"nope\"; using \"brown\""
    );
    assert!(lines[1].ends_with("; using the built-in click"));
}

#[test]
fn edge_brown_chosen_and_failing_writes_one_line_not_two() {
    let tmp = TempDir::new("brown-only");
    make_broken_pack(tmp.path(), "brown");
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(tmp.path()), "brown", &err);
    assert!(is_builtin_click(&sounds));
    let text = written(&buffer);
    assert_eq!(sound_lines(&text).len(), 1, "{text:?}");
    assert!(text.trim_end().ends_with("; using the built-in click"));
}

#[test]
fn ac11_an_unresolved_packs_folder_is_brown_failing() {
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(None, "blue", &err);
    assert!(is_builtin_click(&sounds));
    let text = written(&buffer);
    let lines = sound_lines(&text);
    assert!(!lines.is_empty() && lines.len() <= 2, "{text:?}");
    assert!(
        lines
            .last()
            .unwrap()
            .ends_with("; using the built-in click")
    );
}

// AC13

#[test]
fn ac13_the_pack_is_chosen_before_the_listener_starts() {
    let (_, err) = fake_err();
    let sounds = choose_sounds(Some(&bundled()), "typewriter", &err);
    let sink = Arc::new(RecordingSink::default());
    run_sounds(
        |mut consumer| {
            // The first event already plays a typewriter sample: enter is
            // the long carriage return.
            consumer(down(KeyCode::Enter));
            Ok(Box::new(|| Ok(())) as Wait)
        },
        sink.clone(),
        sounds,
        err,
    );
    let played = sink.samples();
    assert_eq!(played.len(), 1);
    let typewriter = packs::load(&bundled().join("typewriter")).unwrap();
    assert!(
        played_by(&typewriter.sounds).contains(
            &played[0]
                .samples()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<u32>>()
        )
    );
}

// AC14

#[test]
fn ac14_mks_pack_is_ignored_when_choosing_the_pack() {
    let tmp = TempDir::new("mkspack");
    make_pack(tmp.path(), "outside", 99);
    // SAFETY: `set_var` is unsafe because another thread may read the
    // environment through libc at the same time. std serialises its own
    // environment access, and no test in this binary reads `MKS_PACK`
    // concurrently (only this test and the source grep name it).
    unsafe { std::env::set_var("MKS_PACK", tmp.path().join("outside")) };
    let (buffer, err) = fake_err();
    let sounds = choose_sounds(Some(&bundled()), "brown", &err);
    unsafe { std::env::remove_var("MKS_PACK") };
    assert_eq!(written(&buffer), "");
    assert!(is_pack(&sounds, "brown"));
}

#[test]
fn ac14_mks_pack_is_not_mentioned_in_the_source() {
    fn walk(dir: &Path, hits: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if p.extension().is_some_and(|x| x == "rs")
                && fs::read_to_string(&p).unwrap().contains("MKS_PACK")
            {
                hits.push(p);
            }
        }
    }
    let mut hits = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut hits,
    );
    assert!(hits.is_empty(), "MKS_PACK still named in {hits:?}");
}
