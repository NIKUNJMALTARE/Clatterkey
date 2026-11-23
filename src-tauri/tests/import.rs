//! Feature 10, the core's import of a pack folder (AC12, AC14 to AC20, AC24
//! half, AC25, AC27), on temporary folders and the fixture packs. No Tauri.
//!
//! Interfaces assumed from the feature 10 plan, all reached through
//! `mechanical_key_sounds_lib::packs` (the plan names the files, not the
//! re-exports):
//!
//! ```ignore
//! Library::new(Option<PathBuf>, Option<PathBuf>), Library::bundled_only(PathBuf),
//! Library::none(); is_bundled(&str), find(&str) -> Option<Found { dir, imported }>,
//! list() -> Vec<Listed { info, imported }>, imported_dir(), clean_temporary()
//! pack_id_for(&str, impl Fn(&str) -> bool) -> String
//! assign(&str) -> (KeyKind, KeyGroup)
//! import(&Path, &Library) -> Result<Imported { info, replaced, sounds }, ImportError>
//! import_with(&Path, &Library, &dyn Fn(&Path, &Path) -> io::Result<()>)
//! ImportError::{Pack(PackError), Copy(String), NoPacksFolder, AlreadyInside}
//! audio::DecodeError::RateTooHigh
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mechanical_key_sounds_lib::audio::{DecodeError, decode};
use mechanical_key_sounds_lib::keys::{KeyGroup, KeyKind};
use mechanical_key_sounds_lib::packs::{
    ImportError, Library, assign, import, import_with, load, pack_id_for,
};
use mechanical_key_sounds_lib::startup::is_pack_id;
use serde_json::Value;

mod common;
use common::*;

/// A library over a bundled folder and an imported folder (which need not
/// exist yet).
fn library(bundled: &Path, imported: &Path) -> Library {
    Library::new(Some(bundled.to_path_buf()), Some(imported.to_path_buf()))
}

/// A bundled folder with `brown` and `blue`, and an imported folder path
/// that does not exist yet.
struct Env {
    _bundled: TempDir,
    data: TempDir,
    lib: Library,
    imported: PathBuf,
}

fn env() -> Env {
    let bundled = packs_folder("bundled", &[("brown", "default-only"), ("blue", "full")]);
    let data = TempDir::new("data");
    let imported = data.path().join("packs");
    let lib = library(bundled.path(), &imported);
    Env {
        _bundled: bundled,
        data,
        lib,
        imported,
    }
}

fn nothing_written(imported: &Path) {
    if imported.exists() {
        let entries: Vec<_> = fs::read_dir(imported)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(entries.is_empty(), "left behind: {entries:?}");
    }
}

fn err_text(e: &ImportError) -> String {
    e.to_string()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {v}"))
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    out.sort();
    out
}

fn sorted(items: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}

// ---- AC12: a valid pack folder ----

#[test]
fn ac12_a_pack_folder_is_copied_with_only_the_files_it_names() {
    let env = env();
    let (_tmp, src) = source_from_fixture("my-clicks", "full");
    fs::write(src.join("notes.txt"), "not named").unwrap();
    fs::create_dir_all(src.join("extra")).unwrap();
    fs::write(src.join("extra/unused.wav"), wav_bytes(44100, 50, 8000)).unwrap();
    let before = snapshot(&src);

    let done = import(&src, &env.lib).expect("imports");

    assert_eq!(done.info.id, "my-clicks");
    assert_eq!(done.info.name, "Full test pack");
    assert_eq!(done.info.author.as_deref(), Some("Test Author"));
    assert_eq!(done.info.license, "CC0-1.0");
    assert!(done.info.has_key_up);
    assert!(!done.replaced);

    let dest = env.imported.join("my-clicks");
    assert_eq!(
        files_under(&dest),
        sorted(&[
            "pack.json",
            "down/a1.wav",
            "down/a2.wav",
            "down/a3.wav",
            "down/space.wav",
            "down/enter.wav",
            "down/back.wav",
            "down/mod.wav",
            "up/a.wav",
            "up/space.wav",
        ])
    );
    let copy = load(&dest).expect("the copy loads");
    assert_eq!(copy.name, "Full test pack");
    for name in ["down/a1.wav", "up/space.wav"] {
        assert_eq!(
            fs::read(dest.join(name)).unwrap(),
            fs::read(src.join(name)).unwrap()
        );
    }
    assert_eq!(snapshot(&src), before, "the source is unchanged");
}

#[test]
fn ac12_the_data_folder_gets_only_the_pack_folder() {
    let env = env();
    let (_tmp, src) = source_from_fixture("my-clicks", "default-only");
    import(&src, &env.lib).unwrap();
    let entries: Vec<_> = fs::read_dir(&env.imported)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, ["my-clicks"], "no temporary folder is left");
}

#[test]
fn ac12_extra_top_level_fields_in_pack_json_are_kept_in_the_copy() {
    let env = env();
    let (_tmp, src) = source_from_fixture("extra", "extra-field");
    import(&src, &env.lib).unwrap();
    let copy = read_json(&env.imported.join("extra/pack.json"));
    assert_eq!(copy["version"], 7);
    assert_eq!(copy["homepage"], "https://example.invalid");
    assert_eq!(
        fs::read(env.imported.join("extra/pack.json")).unwrap(),
        fs::read(src.join("pack.json")).unwrap()
    );
}

#[test]
fn ac12_a_folder_that_does_not_exist_is_refused_and_writes_nothing() {
    let env = env();
    let missing = env.data.path().join("nowhere");
    let err = import(&missing, &env.lib).expect_err("refused");
    assert!(matches!(err, ImportError::Pack(_)), "{err}");
    nothing_written(&env.imported);
}

// ---- AC14: ids ----

fn not_bundled(_: &str) -> bool {
    false
}

#[test]
fn ac14_ids_from_folder_names() {
    let bundled = |id: &str| id == "brown";
    assert_eq!(pack_id_for("My Clicks", bundled), "my-clicks");
    assert_eq!(pack_id_for("ÉCOUTE!!", bundled), "coute");
    assert_eq!(pack_id_for("--", bundled), "pack");
    let long = "abcdefghij".repeat(7);
    assert_eq!(pack_id_for(&long, bundled), &long[..40]);
    assert_eq!(pack_id_for("brown", bundled), "brown-2");
}

#[test]
fn ac14_every_id_made_is_a_valid_pack_id() {
    for name in [
        "My Clicks",
        "ÉCOUTE!!",
        "--",
        &"x y".repeat(40),
        "brown",
        "日本語",
        "a/b\\c:d",
        "",
        "  ",
        "-_-",
    ] {
        let id = pack_id_for(name, |id| id == "brown");
        assert!(is_pack_id(&id), "{name:?} made {id:?}");
        assert!(id.len() <= 43, "{id:?}");
    }
}

#[test]
fn ac14_runs_of_other_characters_become_one_dash_and_ends_are_trimmed() {
    assert_eq!(pack_id_for("  Old   Board!! ", not_bundled), "old-board");
    assert_eq!(pack_id_for("a__b--c", not_bundled), "a__b--c");
    assert_eq!(pack_id_for("a.b.c", not_bundled), "a-b-c");
}

#[test]
fn ac14_edge_a_40_character_name_ending_in_a_symbol_is_trimmed_after_the_cut() {
    let name = format!("{}!!!", "x".repeat(39));
    assert_eq!(pack_id_for(&name, not_bundled), "x".repeat(39));
    let name = format!("{} b", "a".repeat(39));
    assert_eq!(pack_id_for(&name, not_bundled), "a".repeat(39));
}

#[test]
fn ac14_edge_a_name_that_becomes_a_bundled_id_after_cleaning_gets_a_number() {
    assert_eq!(pack_id_for("Brown!", |id| id == "brown"), "brown-2");
    assert_eq!(
        pack_id_for("brown", |id| id == "brown" || id == "brown-2"),
        "brown-3"
    );
}

#[test]
fn ac14_a_user_folder_called_brown_is_imported_as_brown_2() {
    let env = env();
    let (_tmp, src) = source_from_fixture("brown", "full");
    let done = import(&src, &env.lib).unwrap();
    assert_eq!(done.info.id, "brown-2");
    assert!(env.imported.join("brown-2/pack.json").is_file());
    assert!(!env.imported.join("brown").exists());
}

// ---- assign ----

#[test]
fn ac15_assign_follows_the_file_name_rules() {
    use KeyGroup::*;
    use KeyKind::*;
    let cases: [(&str, (KeyKind, KeyGroup)); 17] = [
        ("a1", (Down, Default)),
        ("a2", (Down, Default)),
        ("space", (Down, Space)),
        ("Space 2", (Down, Space)),
        ("enter", (Down, Enter)),
        ("backspace1", (Down, Backspace)),
        ("modifier", (Down, Modifier)),
        ("enter-up", (Up, Enter)),
        ("click-up", (Up, Default)),
        ("click_up", (Up, Default)),
        ("click up", (Up, Default)),
        ("backspace-up", (Up, Backspace)),
        ("SPACE_UP", (Up, Space)),
        ("up", (Down, Default)),
        ("spacebar", (Down, Space)),
        ("-up", (Up, Default)),
        ("shift", (Down, Default)),
    ];
    for (stem, expected) in cases {
        assert_eq!(assign(stem), expected, "{stem:?}");
    }
}

#[test]
fn ac15_assign_has_no_other_aliases() {
    use KeyGroup::Default;
    use KeyKind::Down;
    for stem in ["return", "shift", "tab", "ctrl", "key-space"] {
        assert_eq!(assign(stem), (Down, Default), "{stem:?}");
    }
}

// ---- AC15: a plain folder ----

fn plain_folder(name: &str) -> (TempDir, PathBuf) {
    let tmp = TempDir::new("plain");
    let dir = tmp.path().join(name);
    fs::create_dir_all(&dir).unwrap();
    (tmp, dir)
}

fn put(dir: &Path, name: &str, fixture: &str) {
    fs::write(dir.join(name), fs::read(fixture_audio(fixture)).unwrap()).unwrap();
}

#[test]
fn ac15_a_plain_folder_is_wrapped_in_a_pack() {
    let env = env();
    let (_tmp, src) = plain_folder("Old Board");
    put(&src, "a1.wav", "tone-mono-44100.wav");
    put(&src, "A2.WAV", "tone-stereo-44100.wav");
    put(&src, "space.wav", "tone-mono-48000.wav");
    put(&src, "Space 2.flac", "tone-mono-44100.flac");
    put(&src, "enter-up.mp3", "tone-mono-44100.mp3");
    put(&src, "click_up.ogg", "tone-stereo-44100.ogg");
    fs::write(src.join("notes.txt"), "hello").unwrap();
    fs::write(src.join(".DS_Store"), "junk").unwrap();
    fs::create_dir_all(src.join("sub")).unwrap();
    put(&src.join("sub"), "b.wav", "tone-mono-44100.wav");
    let before = snapshot(&src);

    let done = import(&src, &env.lib).expect("imports");
    assert_eq!(done.info.id, "old-board");
    assert_eq!(done.info.name, "Old Board");
    assert_eq!(done.info.author, None);
    assert_eq!(done.info.license, "Licence not stated");
    assert!(done.info.has_key_up);

    let dest = env.imported.join("old-board");
    let manifest = read_json(&dest.join("pack.json"));
    assert_eq!(
        strings(&manifest["keys"]["default"]),
        sorted(&["a1.wav", "A2.WAV"])
    );
    assert_eq!(
        strings(&manifest["keys"]["space"]),
        sorted(&["space.wav", "Space 2.flac"])
    );
    assert_eq!(strings(&manifest["up"]["enter"]), ["enter-up.mp3"]);
    assert_eq!(strings(&manifest["up"]["default"]), ["click_up.ogg"]);
    assert_eq!(manifest["name"], "Old Board");
    assert!(manifest.get("author").is_none_or(Value::is_null));
    assert_eq!(manifest["license"], "Licence not stated");

    assert_eq!(
        files_under(&dest),
        sorted(&[
            "pack.json",
            "a1.wav",
            "A2.WAV",
            "space.wav",
            "Space 2.flac",
            "enter-up.mp3",
            "click_up.ogg",
        ])
    );
    assert!(load(&dest).is_ok(), "the copy loads");
    assert_eq!(snapshot(&src), before, "the source is unchanged");
}

#[test]
fn ac15_the_folder_name_as_written_is_the_pack_name_trimmed_and_cut_to_60() {
    let env = env();
    let name = format!("{} tail", "N".repeat(58));
    let (_tmp, src) = plain_folder(&name);
    put(&src, "a.wav", "tone-mono-44100.wav");
    let done = import(&src, &env.lib).unwrap();
    assert_eq!(done.info.name.chars().count(), 60);
    assert_eq!(
        done.info.name,
        name.chars().take(60).collect::<String>().trim()
    );
}

#[test]
fn ac15_the_extension_is_matched_in_any_case_and_the_format_comes_from_the_content() {
    let env = env();
    let (_tmp, src) = plain_folder("Cases");
    put(&src, "x.OGG", "tone-stereo-44100.ogg");
    put(&src, "y.Mp3", "tone-mono-44100.mp3");
    let done = import(&src, &env.lib).unwrap();
    assert_eq!(done.info.id, "cases");
    let manifest = read_json(&env.imported.join("cases/pack.json"));
    assert_eq!(
        strings(&manifest["keys"]["default"]),
        sorted(&["x.OGG", "y.Mp3"])
    );
}

#[test]
fn ac15_a_plain_folder_with_only_up_and_space_ordinary_sound_present_imports() {
    let env = env();
    let (_tmp, src) = plain_folder("Mix");
    put(&src, "thock.wav", "tone-mono-44100.wav");
    put(&src, "space.wav", "tone-mono-48000.wav");
    let done = import(&src, &env.lib).unwrap();
    assert!(!done.info.has_key_up);
}

// ---- AC16: a plain folder that is refused ----

const NO_AUDIO: &str = "no audio files found (WAV, OGG, MP3 or FLAC)";
const NO_ORDINARY: &str = "no sound for ordinary keys; at least one file must not be named for space, enter, backspace, modifier or -up";

#[test]
fn ac16_a_folder_with_no_usable_audio_file_is_refused() {
    let env = env();
    let (_tmp, src) = plain_folder("Old Board");
    fs::write(src.join("notes.txt"), "hello").unwrap();
    fs::write(src.join(".hidden.wav"), wav_bytes(44100, 50, 8000)).unwrap();
    fs::create_dir_all(src.join("sub")).unwrap();
    put(&src.join("sub"), "b.wav", "tone-mono-44100.wav");
    let err = import(&src, &env.lib).expect_err("refused");
    assert_eq!(err_text(&err), NO_AUDIO);
    nothing_written(&env.imported);
}

#[test]
fn ac16_an_empty_folder_is_refused() {
    let env = env();
    let (_tmp, src) = plain_folder("Empty");
    let err = import(&src, &env.lib).expect_err("refused");
    assert_eq!(err_text(&err), NO_AUDIO);
    nothing_written(&env.imported);
}

#[test]
fn ac16_a_folder_with_no_ordinary_key_down_sound_is_refused() {
    let env = env();
    let (_tmp, src) = plain_folder("Old Board");
    put(&src, "space.wav", "tone-mono-44100.wav");
    let err = import(&src, &env.lib).expect_err("refused");
    assert_eq!(err_text(&err), NO_ORDINARY);
    nothing_written(&env.imported);

    // Only key-up sounds and named groups is the same case.
    put(&src, "click-up.wav", "tone-mono-44100.wav");
    put(&src, "enter.wav", "tone-mono-44100.wav");
    let err = import(&src, &env.lib).expect_err("refused");
    assert_eq!(err_text(&err), NO_ORDINARY);
    nothing_written(&env.imported);
}

#[test]
fn ac16_101_audio_files_are_refused_and_100_are_not() {
    let env = env();
    let (_tmp, src) = plain_folder("Music");
    let bytes = wav_bytes(8000, 100, 1000);
    for i in 0..101 {
        fs::write(src.join(format!("k{i:03}.wav")), &bytes).unwrap();
    }
    let err = import(&src, &env.lib).expect_err("refused");
    assert_eq!(err_text(&err), "uses more than 100 different audio files");
    nothing_written(&env.imported);

    fs::remove_file(src.join("k100.wav")).unwrap();
    assert!(import(&src, &env.lib).is_ok(), "100 files are fine");
}

#[test]
fn ac16_one_bad_file_refuses_the_whole_folder_and_the_message_names_it() {
    let env = env();
    let (_tmp, src) = plain_folder("Old Board");
    put(&src, "a1.wav", "tone-mono-44100.wav");
    put(&src, "thock 3.wav", "long-4s.wav");
    let err = import(&src, &env.lib).expect_err("refused");
    assert_eq!(err_text(&err), "\"thock 3.wav\": longer than 3 seconds");
    nothing_written(&env.imported);
}

#[test]
fn ac16_a_file_that_is_not_audio_or_is_silent_is_refused_by_name() {
    let env = env();
    for (fixture, fragment) in [("not-audio.txt", "bad.wav"), ("silent.wav", "bad.wav")] {
        let (_tmp, src) = plain_folder("Old Board");
        put(&src, "a1.wav", "tone-mono-44100.wav");
        put(&src, "bad.wav", fixture);
        let msg = err_text(&import(&src, &env.lib).expect_err("refused"));
        assert!(msg.contains(fragment), "{msg}");
        assert!(!msg.contains("pack \""), "no prefix: {msg}");
        nothing_written(&env.imported);
    }
}

#[cfg(unix)]
#[test]
fn ac16_edge_a_link_to_a_file_outside_the_plain_folder_is_refused() {
    let env = env();
    let (tmp, src) = plain_folder("Linked");
    put(&src, "a1.wav", "tone-mono-44100.wav");
    let outside = tmp.path().join("outside.wav");
    fs::write(
        &outside,
        fs::read(fixture_audio("tone-mono-44100.wav")).unwrap(),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, src.join("evil.wav")).unwrap();
    let msg = err_text(&import(&src, &env.lib).expect_err("refused"));
    assert!(msg.contains("evil.wav"), "{msg}");
    assert!(msg.contains("outside the pack folder"), "{msg}");
    nothing_written(&env.imported);
}

// ---- AC17: invalid pack folders ----

/// What `load` says about `dir`, without its `pack "<id>": ` prefix.
fn loader_reason(dir: &Path) -> String {
    let e = load(dir).expect_err("the loader refuses it");
    let text = e.to_string();
    let prefix = format!("pack {:?}: ", e.pack);
    text.strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("unexpected message {text}"))
        .to_string()
}

fn invalid_pack_folders() -> Vec<(&'static str, TestPack)> {
    let mut out = Vec::new();

    let p = TestPack::new("my-pack", &json_default(&["down/a.wav"]));
    out.push(("file not found", p));

    out.push((
        "not json",
        TestPack::new(
            "my-pack",
            "{\n  \"name\": \"x\",\n  \"license\": \"CC0\" \"keys\": {}\n}",
        ),
    ));
    out.push((
        "no name",
        TestPack::new(
            "my-pack",
            r#"{"license":"CC0-1.0","keys":{"default":["a.wav"]}}"#,
        ),
    ));
    out.push((
        "empty default",
        TestPack::new(
            "my-pack",
            r#"{"name":"T","license":"CC0-1.0","keys":{"default":[]}}"#,
        ),
    ));
    out.push((
        "dot dot",
        TestPack::new("my-pack", &json_default(&["../outside.wav"])),
    ));
    out.push((
        "absolute",
        TestPack::new("my-pack", &json_default(&["/etc/passwd"])),
    ));
    out.push((
        "unknown group",
        TestPack::new(
            "my-pack",
            r#"{"name":"T","license":"CC0-1.0","keys":{"default":["a.wav"],"tab":["a.wav"]}}"#,
        ),
    ));

    let p = TestPack::new("my-pack", &json_default(&["a.wav"]));
    p.copy_fixture("a.wav", "not-audio.txt");
    out.push(("not audio", p));

    let p = TestPack::new("my-pack", &json_default(&["a.wav"]));
    p.copy_fixture("a.wav", "silent.wav");
    out.push(("silent", p));

    let p = TestPack::new("my-pack", &json_default(&["a.wav"]));
    p.copy_fixture("a.wav", "long-4s.wav");
    out.push(("too long", p));

    let names: Vec<String> = (0..101).map(|i| format!("k{i:03}.wav")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let p = TestPack::new("my-pack", &json_default(&refs));
    for n in &names {
        p.add(n, &wav_bytes(8000, 100, 1000));
    }
    out.push(("too many files", p));

    out
}

#[test]
fn ac17_each_invalid_pack_folder_is_refused_with_the_loaders_message_and_writes_nothing() {
    for (label, pack) in invalid_pack_folders() {
        let env = env();
        let expected = loader_reason(&pack.dir);
        let err = import(&pack.dir, &env.lib)
            .err()
            .unwrap_or_else(|| panic!("{label}: expected a refusal"));
        assert!(matches!(err, ImportError::Pack(_)), "{label}: {err}");
        assert_eq!(err_text(&err), expected, "{label}");
        assert!(!expected.starts_with("pack "), "{label}: {expected}");
        nothing_written(&env.imported);
    }
}

#[test]
fn ac17_the_spec_examples_read_as_written() {
    let env = env();
    let p = TestPack::new(
        "my-pack",
        r#"{"name":"T","license":"CC0-1.0","keys":{"default":["a.wav"],"space":["down/space.wav"]}}"#,
    );
    p.copy_fixture("a.wav", "tone-mono-44100.wav");
    assert_eq!(
        err_text(&import(&p.dir, &env.lib).err().unwrap()),
        "keys.space[0] \"down/space.wav\": file not found"
    );
    let q = TestPack::new(
        "my-pack",
        "{\n  \"name\": \"x\",\n  \"license\": \"CC0\" \"keys\": {}\n}",
    );
    assert!(
        err_text(&import(&q.dir, &env.lib).err().unwrap())
            .starts_with("pack.json: not valid JSON (line "),
    );
}

#[test]
fn ac17_a_refused_import_does_not_touch_a_pack_already_there() {
    let env = env();
    let (_t1, good) = source_from_fixture("my-pack", "default-only");
    import(&good, &env.lib).unwrap();
    let before = snapshot(&env.imported);
    let bad = TestPack::new("other", &json_default(&["nope.wav"]));
    assert!(import(&bad.dir, &env.lib).is_err());
    assert_eq!(snapshot(&env.imported), before);
}

// ---- AC18: importing again ----

#[test]
fn ac18_the_same_id_replaces_the_copy_whole() {
    let env = env();
    let (_t1, first) = source_from_fixture("my-clicks", "full");
    let r1 = import(&first, &env.lib).unwrap();
    assert!(!r1.replaced);
    assert!(env.imported.join("my-clicks/up/space.wav").is_file());

    // A different folder with the same name.
    let (_t2, second) = source_from_fixture("my-clicks", "default-only");
    let r2 = import(&second, &env.lib).unwrap();
    assert!(r2.replaced);
    assert_eq!(r2.info.name, "Default only");
    let dest = env.imported.join("my-clicks");
    assert_eq!(
        files_under(&dest),
        sorted(&["pack.json", "down/a1.wav", "down/a2.wav", "down/a3.wav"]),
        "no file of the old copy remains"
    );
    let entries = fs::read_dir(&env.imported).unwrap().count();
    assert_eq!(entries, 1, "no old or temporary folder is left");
}

#[test]
fn ac18_a_name_that_makes_the_same_id_replaces_too() {
    let env = env();
    let (_t1, first) = source_from_fixture("My Clicks", "full");
    let (_t2, second) = source_from_fixture("my clicks!", "default-only");
    assert!(!import(&first, &env.lib).unwrap().replaced);
    assert!(import(&second, &env.lib).unwrap().replaced);
    assert_eq!(fs::read_dir(&env.imported).unwrap().count(), 1);
}

#[test]
fn ac18_an_invalid_replacement_leaves_the_old_copy_exactly_as_it_was() {
    let env = env();
    let (_t1, first) = source_from_fixture("my-clicks", "full");
    import(&first, &env.lib).unwrap();
    let before = snapshot(&env.imported);

    let bad = TestPack::new("my-clicks", &json_default(&["missing.wav"]));
    assert!(import(&bad.dir, &env.lib).is_err());
    assert_eq!(snapshot(&env.imported), before);
    assert!(load(&env.imported.join("my-clicks")).is_ok());
}

#[test]
fn ac18_a_bundled_pack_is_never_replaced() {
    let env = env();
    let bundled_before = snapshot(env.lib_bundled());
    let (_t, src) = source_from_fixture("blue", "default-only");
    let done = import(&src, &env.lib).unwrap();
    assert_eq!(done.info.id, "blue-2");
    assert!(!done.replaced);
    assert_eq!(snapshot(env.lib_bundled()), bundled_before);
}

impl Env {
    fn lib_bundled(&self) -> &Path {
        self._bundled.path()
    }
}

#[test]
fn ac18_edge_importing_the_apps_own_copy_is_refused_and_the_copy_survives() {
    let env = env();
    let (_t, src) = source_from_fixture("my-clicks", "full");
    import(&src, &env.lib).unwrap();
    let own = env.imported.join("my-clicks");
    let before = snapshot(&env.imported);
    let err = import(&own, &env.lib).expect_err("refused");
    assert!(matches!(err, ImportError::AlreadyInside), "{err}");
    assert_eq!(
        err_text(&err),
        "this folder is the app's own copy of the pack"
    );
    assert_eq!(snapshot(&env.imported), before);
    assert!(load(&own).is_ok());
}

// ---- AC19: a copy that fails part-way ----

fn failing_on(nth: usize) -> impl Fn(&Path, &Path) -> io::Result<()> {
    let count = AtomicUsize::new(0);
    move |from, to| {
        if count.fetch_add(1, Ordering::SeqCst) + 1 == nth {
            Err(io::Error::other("disk full"))
        } else {
            fs::copy(from, to).map(|_| ())
        }
    }
}

#[test]
fn ac19_a_copy_that_fails_part_way_is_refused_and_leaves_nothing() {
    for nth in [1, 3, 9] {
        let env = env();
        let (_t, src) = source_from_fixture("my-clicks", "full");
        let err = import_with(&src, &env.lib, &failing_on(nth))
            .err()
            .unwrap_or_else(|| panic!("copy {nth} fails"));
        assert!(matches!(err, ImportError::Copy(_)), "{err}");
        let msg = err_text(&err);
        assert!(msg.starts_with("could not be copied: "), "{msg}");
        assert!(msg.contains("disk full"), "{msg}");
        assert!(!env.imported.join("my-clicks").exists());
        assert!(env.lib.list().iter().all(|l| !l.imported));
        nothing_written(&env.imported);
    }
}

#[test]
fn ac19_a_failed_copy_over_an_existing_pack_changes_nothing() {
    let env = env();
    let (_t1, first) = source_from_fixture("my-clicks", "full");
    import(&first, &env.lib).unwrap();
    let before = snapshot(&env.imported);
    let (_t2, second) = source_from_fixture("my-clicks", "default-only");
    let err = import_with(&second, &env.lib, &failing_on(2))
        .err()
        .unwrap();
    assert!(matches!(err, ImportError::Copy(_)), "{err}");
    assert_eq!(snapshot(&env.imported), before);
    assert!(load(&env.imported.join("my-clicks")).is_ok());
}

#[test]
fn ac19_a_destination_that_cannot_be_written_is_a_copy_error() {
    let env = env();
    // The imported folder's parent is a file, so it cannot be made.
    fs::write(env.data.path().join("packs"), "a file").unwrap();
    let (_t, src) = source_from_fixture("my-clicks", "default-only");
    let err = import(&src, &env.lib).expect_err("refused");
    assert!(err_text(&err).starts_with("could not be copied: "), "{err}");
}

#[test]
fn ac19_with_no_imported_folder_known_the_import_is_refused() {
    let bundled = packs_folder("bundled", &[("brown", "default-only")]);
    let lib = Library::bundled_only(bundled.path().to_path_buf());
    let (_t, src) = source_from_fixture("my-clicks", "default-only");
    let err = import(&src, &lib).expect_err("refused");
    assert!(matches!(err, ImportError::NoPacksFolder), "{err}");
    assert_eq!(err_text(&err), "the app data folder was not found");
}

// ---- AC20: the library lists both folders ----

#[test]
fn ac20_both_folders_are_listed_sorted_by_name_with_the_imported_flag() {
    let env = env();
    let (_t, src) = source_from_fixture("my-clicks", "extra-field");
    import(&src, &env.lib).unwrap();
    let listed = env.lib.list();
    let rows: Vec<(String, String, bool)> = listed
        .iter()
        .map(|l| (l.info.id.clone(), l.info.name.clone(), l.imported))
        .collect();
    assert_eq!(
        rows,
        [
            ("brown".to_string(), "Default only".to_string(), false),
            ("my-clicks".to_string(), "Extra field".to_string(), true),
            ("blue".to_string(), "Full test pack".to_string(), false),
        ]
    );
}

#[test]
fn ac20_invalid_ids_shadows_temporary_folders_and_unreadable_packs_are_not_listed() {
    let env = env();
    fs::create_dir_all(&env.imported).unwrap();
    copy_dir(&fixture_pack("full"), &env.imported.join("has space"));
    copy_dir(&fixture_pack("extra-field"), &env.imported.join("brown"));
    copy_dir(&fixture_pack("full"), &env.imported.join(".import-x-1-1"));
    copy_dir(&fixture_pack("full"), &env.imported.join(".old-y-2"));
    fs::create_dir_all(env.imported.join("garbled")).unwrap();
    fs::write(env.imported.join("garbled/pack.json"), "{ nope").unwrap();
    fs::write(env.imported.join("stray.txt"), "x").unwrap();
    copy_dir(&fixture_pack("default-only"), &env.imported.join("by-hand"));

    let ids: Vec<String> = env.lib.list().into_iter().map(|l| l.info.id).collect();
    assert_eq!(ids, ["brown", "by-hand", "blue"]);
    // The bundled one is the one listed under "brown".
    let brown = env
        .lib
        .list()
        .into_iter()
        .find(|l| l.info.id == "brown")
        .unwrap();
    assert!(!brown.imported);
    assert_eq!(brown.info.name, "Default only");
}

#[test]
fn ac20_an_absent_imported_folder_is_not_an_error() {
    let env = env();
    assert!(!env.imported.exists());
    assert_eq!(env.lib.list().len(), 2);
    assert!(Library::none().list().is_empty());
}

#[test]
fn ac20_find_looks_in_the_bundled_folder_first_then_the_imported_one() {
    let env = env();
    fs::create_dir_all(&env.imported).unwrap();
    copy_dir(&fixture_pack("extra-field"), &env.imported.join("brown"));
    copy_dir(&fixture_pack("full"), &env.imported.join("mine"));

    let brown = env.lib.find("brown").expect("found");
    assert!(!brown.imported);
    assert_eq!(brown.dir, env._bundled.path().join("brown"));
    let mine = env.lib.find("mine").expect("found");
    assert!(mine.imported);
    assert_eq!(mine.dir, env.imported.join("mine"));
    assert!(env.lib.find("nope").is_none());
    for bad in ["../x", "/etc", "", "..", "a b"] {
        assert!(env.lib.find(bad).is_none(), "{bad:?}");
    }
    assert!(env.lib.is_bundled("brown"));
    assert!(!env.lib.is_bundled("mine"));
    assert!(!env.lib.is_bundled("../brown"));
    assert_eq!(env.lib.imported_dir(), Some(env.imported.as_path()));
    assert_eq!(Library::none().imported_dir(), None);
}

#[test]
fn ac20_edge_clean_temporary_removes_leftovers_and_restores_a_lost_pack() {
    let env = env();
    let imported = &env.imported;
    fs::create_dir_all(imported).unwrap();
    for name in [
        ".import-a-1-1",
        ".removed-b-1-2",
        ".old-gone-3",
        ".old-kept-4",
    ] {
        copy_dir(&fixture_pack("default-only"), &imported.join(name));
    }
    copy_dir(&fixture_pack("full"), &imported.join("kept"));
    copy_dir(&fixture_pack("full"), &imported.join("fine"));

    env.lib.clean_temporary();

    let mut names: Vec<String> = fs::read_dir(imported)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["fine", "gone", "kept"]);
    assert!(load(&imported.join("gone")).is_ok(), "restored whole");
    assert_eq!(
        read_json(&imported.join("kept/pack.json"))["name"],
        "Full test pack",
        "the existing pack is not replaced by the old one"
    );
}

#[test]
fn ac20_clean_temporary_with_no_imported_folder_does_nothing() {
    let env = env();
    env.lib.clean_temporary();
    Library::none().clean_temporary();
    assert!(!env.imported.exists());
}

// ---- AC24 (library half): ids are checked before any path is built ----

#[test]
fn ac24_an_imported_folder_is_never_reached_through_an_invalid_id() {
    let env = env();
    let (_t, src) = source_from_fixture("my-clicks", "default-only");
    import(&src, &env.lib).unwrap();
    assert!(env.lib.find("my-clicks").is_some());
    assert!(env.lib.find("my-clicks/..").is_none());
    assert!(env.lib.find("./my-clicks").is_none());
}

// ---- AC25: the 96 kHz limit ----

const THREE_SECONDS: Duration = Duration::from_secs(3);

#[test]
fn ac25_a_file_above_96_khz_is_refused_by_decode() {
    for rate in [96_001, 192_000] {
        let err = decode(wav_bytes(rate, 100, 8000), THREE_SECONDS)
            .err()
            .unwrap_or_else(|| panic!("{rate} Hz is refused"));
        assert!(matches!(err, DecodeError::RateTooHigh), "{rate}: {err}");
        assert_eq!(err.to_string(), "sample rate above 96 kHz");
    }
}

#[test]
fn ac25_96_khz_still_loads() {
    let sample = decode(wav_bytes(96_000, 100, 8000), THREE_SECONDS).expect("96 kHz decodes");
    assert!(sample.duration() > Duration::ZERO);
}

#[test]
fn ac25_a_pack_with_a_192_khz_file_is_refused_naming_the_file() {
    let pack = TestPack::new("hi-rate", &json_default(&["down/hi.wav"]));
    pack.add("down/hi.wav", &wav_bytes(192_000, 100, 8000));
    let text = load(&pack.dir).expect_err("refused").to_string();
    assert!(text.starts_with("pack \"hi-rate\": "), "{text}");
    assert!(text.contains("down/hi.wav"), "{text}");
    assert!(text.ends_with("sample rate above 96 kHz"), "{text}");
}

#[test]
fn ac25_a_pack_with_a_96_khz_file_loads() {
    let pack = TestPack::new("ok-rate", &json_default(&["down/hi.wav"]));
    pack.add("down/hi.wav", &wav_bytes(96_000, 100, 8000));
    assert!(load(&pack.dir).is_ok());
}

#[test]
fn ac25_a_plain_folder_with_a_192_khz_file_is_refused_naming_it() {
    let env = env();
    let (_t, src) = plain_folder("Hi Rate");
    fs::write(src.join("hi.wav"), wav_bytes(192_000, 100, 8000)).unwrap();
    let msg = err_text(&import(&src, &env.lib).expect_err("refused"));
    assert_eq!(msg, "\"hi.wav\": sample rate above 96 kHz");
    nothing_written(&env.imported);
}

#[test]
fn ac25_the_four_bundled_packs_still_load() {
    let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/packs");
    for id in ["blue", "brown", "red", "typewriter"] {
        load(&bundled.join(id)).unwrap_or_else(|e| panic!("{e}"));
    }
}

// ---- AC27: no new capability for the window ----

#[test]
fn ac27_the_window_capabilities_grant_only_core_default() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut seen = 0;
    for entry in fs::read_dir(&dir).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "json") {
            seen += 1;
            let value = read_json(&entry.path());
            let permissions: Vec<&str> = value["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().expect("a plain permission name"))
                .collect();
            assert_eq!(permissions, ["core:default"], "{}", entry.path().display());
        }
    }
    assert!(seen >= 1);
}

// ---- Review follow-ups ----

#[cfg(unix)]
#[test]
fn ac16_a_plain_file_name_with_a_colon_or_backslash_refuses_the_whole_folder() {
    for bad in ["1:2 click.wav", "back\\slash.wav"] {
        let env = env();
        let (_t, src) = plain_folder("Old Board");
        put(&src, "a.wav", "tone-mono-44100.wav");
        put(&src, bad, "tone-mono-44100.wav");
        let err = import(&src, &env.lib).expect_err("refused");
        assert!(matches!(err, ImportError::Pack(_)), "{err}");
        assert_eq!(
            err_text(&err),
            format!("{bad:?}: file names cannot contain : or \\; rename the file")
        );
        nothing_written(&env.imported);
    }
}

#[cfg(unix)]
#[test]
fn ac16_a_non_audio_file_with_a_colon_in_its_name_is_just_ignored() {
    let env = env();
    let (_t, src) = plain_folder("Old Board");
    put(&src, "a.wav", "tone-mono-44100.wav");
    fs::write(src.join("notes:1.txt"), "hi").unwrap();
    import(&src, &env.lib).expect("imports");
    assert_eq!(
        files_under(&env.imported.join("old-board")),
        sorted(&["a.wav", "pack.json"])
    );
}

fn copy_writing(name: &'static str, bytes: Vec<u8>) -> impl Fn(&Path, &Path) -> io::Result<()> {
    move |from, to| {
        if to.file_name().is_some_and(|n| n == name) {
            fs::write(to, &bytes)
        } else {
            fs::copy(from, to).map(|_| ())
        }
    }
}

#[test]
fn ac19_a_copy_that_does_not_hold_audio_is_refused_and_leaves_nothing() {
    let env = env();
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    let err = import_with(
        &src,
        &env.lib,
        &copy_writing("a.wav", b"not audio".to_vec()),
    )
    .expect_err("refused");
    assert!(matches!(err, ImportError::Copy(_)), "{err}");
    assert_eq!(
        err_text(&err),
        "could not be copied: the folder changed while it was being copied"
    );
    nothing_written(&env.imported);
}

#[test]
fn ac19_a_bad_copy_over_an_existing_pack_leaves_it_unchanged() {
    let env = env();
    let (_t1, first) = source_with_lengths("my-clicks", 55, None);
    import(&first, &env.lib).unwrap();
    let before = snapshot(&env.imported);
    let (_t2, second) = source_with_lengths("my-clicks", 33, None);
    let err = import_with(
        &second,
        &env.lib,
        &copy_writing("a.wav", b"not audio".to_vec()),
    )
    .expect_err("refused");
    assert!(matches!(err, ImportError::Copy(_)), "{err}");
    assert_eq!(snapshot(&env.imported), before);
}

#[test]
fn ac12_what_is_returned_is_the_stored_copy_not_the_source() {
    use mechanical_key_sounds_lib::keys::{KeyGroup, KeyKind};
    let env = env();
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    let done = import_with(
        &src,
        &env.lib,
        &copy_writing("a.wav", wav_bytes(44100, 77, 8000)),
    )
    .expect("imports");
    let played = done.sounds.pick(KeyKind::Down, KeyGroup::Default).unwrap();
    assert_eq!(ms(played), 77, "the sound that plays is the copy's");
    let stored = load(&env.imported.join("my-clicks")).unwrap();
    let on_disk = stored
        .sounds
        .pick(KeyKind::Down, KeyGroup::Default)
        .unwrap();
    assert_eq!(ms(on_disk), 77, "the copy on disk agrees");
}

#[test]
fn ac20_an_imported_folder_with_a_bundled_id_is_not_listed_even_if_the_bundled_pack_is_unreadable()
{
    let env = env();
    fs::write(env.lib_bundled().join("brown/pack.json"), "{ nope").unwrap();
    fs::create_dir_all(&env.imported).unwrap();
    copy_dir(&fixture_pack("extra-field"), &env.imported.join("brown"));
    let ids: Vec<String> = env.lib.list().into_iter().map(|l| l.info.id).collect();
    assert!(!ids.iter().any(|i| i == "brown"), "{ids:?}");
    assert!(env.lib.is_bundled("brown"));
}
