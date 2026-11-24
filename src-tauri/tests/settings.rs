//! Tests for the settings module (feature 05: AC6, AC8, AC9, AC12, AC16).
//!
//! Interfaces assumed from the plan (all `pub`, fields included):
//!
//! ```ignore
//! pub const DEFAULT_PACK: &str = "brown";
//! pub struct Settings { pub enabled: bool, pub pack_id: String, pub volume: f64,
//!     pub pitch_variation: f64, pub key_up_sounds: bool, pub launch_at_login: bool }
//! impl Default for Settings   // true, "brown", 0.8, 0.0, true, false
//! pub struct ReadSettings { pub settings: Settings, pub replaced: Vec<&'static str> }
//! pub enum ReadError { NotFound, Invalid(String) }
//! pub fn read(path: &Path) -> Result<ReadSettings, ReadError>;
//! pub fn save(path: &Path, settings: &Settings) -> io::Result<()>;
//! pub fn load_or_create(path: &Path, err: &ErrorOut) -> Settings;
//! ```
//!
//! Settings and ReadSettings are not assumed to derive Debug or PartialEq, so
//! values are compared field by field.

use std::fs;
use std::path::Path;

use clickclak_lib::settings::{DEFAULT_PACK, ReadError, Settings, load_or_create, read, save};

mod common;
use common::*;

fn assert_defaults(s: &Settings) {
    assert!(s.enabled);
    assert_eq!(s.pack_id, "brown");
    assert_eq!(s.volume, 0.8);
    assert_eq!(s.pitch_variation, 0.0);
    assert!(s.key_up_sounds);
    assert!(!s.launch_at_login);
}

fn write(dir: &TempDir, text: &str) -> std::path::PathBuf {
    let path = dir.path().join("settings.json");
    fs::write(&path, text).unwrap();
    path
}

fn ok(path: &Path) -> (Settings, Vec<&'static str>) {
    match read(path) {
        Ok(r) => (r.settings, r.replaced),
        Err(ReadError::NotFound) => panic!("not found"),
        Err(ReadError::Invalid(why)) => panic!("invalid: {why}"),
    }
}

fn invalid(path: &Path) -> String {
    match read(path) {
        Err(ReadError::Invalid(why)) => why,
        Err(ReadError::NotFound) => panic!("expected Invalid, got NotFound"),
        Ok(_) => panic!("expected Invalid, got Ok"),
    }
}

const FULL: &str = r#"{"enabled":false,"pack_id":"red","volume":0.3,"pitch_variation":0.1,"key_up_sounds":false,"launch_at_login":true}"#;

#[test]
fn defaults_are_the_spec_values() {
    assert_defaults(&Settings::default());
    assert_eq!(DEFAULT_PACK, "brown");
}

#[test]
fn a_valid_file_is_read_with_nothing_replaced() {
    let dir = TempDir::new("s-valid");
    let (s, replaced) = ok(&write(&dir, FULL));
    assert!(!s.enabled);
    assert_eq!(s.pack_id, "red");
    assert_eq!(s.volume, 0.3);
    assert_eq!(s.pitch_variation, 0.1);
    assert!(!s.key_up_sounds);
    assert!(s.launch_at_login);
    assert!(replaced.is_empty());
}

#[test]
fn read_reports_a_missing_file_as_not_found() {
    let dir = TempDir::new("s-nf");
    assert!(matches!(
        read(&dir.path().join("settings.json")),
        Err(ReadError::NotFound)
    ));
}

#[test]
fn edge_boundary_values_are_accepted() {
    let dir = TempDir::new("s-bounds");
    for (v, p) in [(0.0, 0.0), (1.0, 0.3)] {
        let path = write(
            &dir,
            &format!(r#"{{"volume":{v:?},"pitch_variation":{p:?}}}"#),
        );
        let (s, replaced) = ok(&path);
        assert_eq!(s.volume, v);
        assert_eq!(s.pitch_variation, p);
        assert!(!replaced.contains(&"volume") && !replaced.contains(&"pitch_variation"));
    }
}

#[test]
fn edge_an_integer_volume_is_accepted_as_a_float() {
    let dir = TempDir::new("s-int");
    let (s, replaced) = ok(&write(&dir, r#"{"volume":1}"#));
    assert_eq!(s.volume, 1.0);
    assert!(!replaced.contains(&"volume"));
}

#[test]
fn edge_values_just_out_of_range_are_replaced() {
    for (field, text) in [
        ("volume", r#"{"volume":1.01}"#),
        ("volume", r#"{"volume":-0.01}"#),
        ("pitch_variation", r#"{"pitch_variation":0.31}"#),
        ("pitch_variation", r#"{"pitch_variation":-0.1}"#),
    ] {
        let dir = TempDir::new("s-range");
        let (s, replaced) = ok(&write(&dir, text));
        assert!(replaced.contains(&field), "{text}: {replaced:?}");
        assert_defaults(&s);
    }
}

#[test]
fn ac8_mixed_fields_take_defaults_keep_others_and_name_the_bad_ones_in_order() {
    let dir = TempDir::new("s-mixed");
    // volume: wrong type; key_up_sounds: wrong type; pitch_variation:
    // out of range; launch_at_login missing; enabled and pack_id fine;
    // plus an unknown field.
    let text = r#"{"enabled":false,"pack_id":"blue","volume":"loud","pitch_variation":0.9,"key_up_sounds":"yes","extra":42}"#;
    let path = write(&dir, text);
    let (s, replaced) = ok(&path);
    assert!(!s.enabled);
    assert_eq!(s.pack_id, "blue");
    assert_eq!(s.volume, 0.8);
    assert_eq!(s.pitch_variation, 0.0);
    assert!(s.key_up_sounds);
    assert!(!s.launch_at_login);
    assert_eq!(
        replaced,
        vec![
            "volume",
            "pitch_variation",
            "key_up_sounds",
            "launch_at_login"
        ]
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), text, "file untouched");
}

#[test]
fn ac8_a_pack_id_of_the_wrong_type_is_replaced_and_any_string_is_kept() {
    let dir = TempDir::new("s-packid");
    let (s, replaced) = ok(&write(&dir, r#"{"pack_id":7}"#));
    assert_eq!(s.pack_id, "brown");
    assert!(replaced.contains(&"pack_id"));

    let (s, replaced) = ok(&write(&dir, r#"{"pack_id":"../nope"}"#));
    assert_eq!(
        s.pack_id, "../nope",
        "pack ids are checked when loading the pack"
    );
    assert!(!replaced.contains(&"pack_id"));
}

#[test]
fn ac8_load_or_create_writes_one_line_naming_the_replaced_fields() {
    let dir = TempDir::new("s-line");
    let text = r#"{"volume":5,"key_up_sounds":1,"pack_id":"red","enabled":true,"pitch_variation":0.1,"launch_at_login":false}"#;
    let path = write(&dir, text);
    let (buffer, err) = fake_err();
    let s = load_or_create(&path, &err);
    assert_eq!(s.pack_id, "red");
    assert_eq!(s.volume, 0.8);
    assert!(s.key_up_sounds);
    assert_eq!(
        written(&buffer),
        "settings: invalid volume, key_up_sounds; using defaults for those\n"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        text,
        "file not rewritten"
    );
}

#[test]
fn ac8_a_fully_valid_file_prints_nothing() {
    let dir = TempDir::new("s-quiet");
    let path = write(&dir, FULL);
    let (buffer, err) = fake_err();
    let s = load_or_create(&path, &err);
    assert_eq!(s.pack_id, "red");
    assert_eq!(written(&buffer), "");
    assert_eq!(fs::read_to_string(&path).unwrap(), FULL);
}

#[test]
fn ac9_invalid_json_and_a_json_array_are_invalid_and_leave_the_file_alone() {
    for text in ["{ not json", "[]", "[1,2]", "\"text\"", "42", ""] {
        let dir = TempDir::new("s-bad");
        let path = write(&dir, text);
        assert!(!invalid(&path).is_empty());

        let (buffer, err) = fake_err();
        let s = load_or_create(&path, &err);
        assert_defaults(&s);
        let out = written(&buffer);
        assert_eq!(out.lines().count(), 1, "{text:?}: {out:?}");
        assert!(out.ends_with("; using defaults\n"), "{out:?}");
        assert!(
            out.starts_with(&format!("settings: {}: ", path.display())),
            "{out:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), text, "file untouched");
    }
}

#[test]
fn ac9_bad_json_names_the_line_and_column() {
    let dir = TempDir::new("s-linecol");
    let why = invalid(&write(&dir, "{\n  \"volume\": ,\n}"));
    assert!(why.contains("line 2"), "{why}");
}

#[test]
fn edge_a_utf8_bom_is_invalid_json_reported_and_untouched() {
    let dir = TempDir::new("s-bom");
    let path = dir.path().join("settings.json");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(FULL.as_bytes());
    fs::write(&path, &bytes).unwrap();
    assert!(!invalid(&path).is_empty());
    let (buffer, err) = fake_err();
    assert_defaults(&load_or_create(&path, &err));
    assert_eq!(written(&buffer).lines().count(), 1);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn ac9_an_unreadable_file_is_invalid_with_defaults() {
    let dir = TempDir::new("s-unreadable");
    // A folder where the file should be: it exists but cannot be read.
    let path = dir.path().join("settings.json");
    fs::create_dir(&path).unwrap();
    let (buffer, err) = fake_err();
    assert_defaults(&load_or_create(&path, &err));
    let out = written(&buffer);
    assert_eq!(out.lines().count(), 1, "{out:?}");
    assert!(out.starts_with("settings: "), "{out:?}");
    assert!(out.ends_with("; using defaults\n"), "{out:?}");
    assert!(path.is_dir());
}

#[test]
fn ac6_a_missing_file_is_created_with_every_default_and_nothing_is_printed() {
    let dir = TempDir::new("s-create");
    // The folder does not exist yet either.
    let path = dir.path().join("config").join("settings.json");
    let (buffer, err) = fake_err();
    let s = load_or_create(&path, &err);
    assert_defaults(&s);
    assert_eq!(written(&buffer), "");

    let (back, replaced) = ok(&path);
    assert_defaults(&back);
    assert!(replaced.is_empty());
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["pack_id"], "brown");
    assert_eq!(json["volume"], 0.8);
}

#[test]
fn ac6_an_unwritable_folder_gives_defaults_and_one_settings_line() {
    let dir = TempDir::new("s-unwritable");
    // A path below a regular file cannot be created.
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let path = blocker.join("settings.json");
    let (buffer, err) = fake_err();
    let s = load_or_create(&path, &err);
    assert_defaults(&s);
    let out = written(&buffer);
    assert_eq!(out.lines().count(), 1, "{out:?}");
    assert!(
        out.starts_with(&format!("settings: could not save {}: ", path.display())),
        "{out:?}"
    );
    assert!(out.ends_with('\n'));
}

#[test]
fn ac12_save_then_read_gives_the_same_values() {
    let dir = TempDir::new("s-roundtrip");
    let path = dir.path().join("settings.json");
    let s = Settings {
        enabled: false,
        pack_id: "typewriter".into(),
        volume: 0.35,
        pitch_variation: 0.25,
        key_up_sounds: false,
        launch_at_login: true,
    };
    save(&path, &s).unwrap();
    let (back, replaced) = ok(&path);
    assert!(replaced.is_empty());
    assert!(!back.enabled);
    assert_eq!(back.pack_id, "typewriter");
    assert_eq!(back.volume, 0.35);
    assert_eq!(back.pitch_variation, 0.25);
    assert!(!back.key_up_sounds);
    assert!(back.launch_at_login);
}

#[test]
fn ac12_save_leaves_no_temporary_file_and_replaces_an_existing_file() {
    let dir = TempDir::new("s-atomic");
    let path = dir.path().join("settings.json");
    fs::write(&path, FULL).unwrap();
    let s = Settings {
        pack_id: "blue".into(),
        ..Default::default()
    };
    save(&path, &s).unwrap();
    assert_eq!(ok(&path).0.pack_id, "blue");
    let names: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["settings.json".to_string()], "no .tmp left");
}

#[test]
fn ac12_save_creates_the_parent_folder() {
    let dir = TempDir::new("s-mkdir");
    let path = dir.path().join("a").join("b").join("settings.json");
    save(&path, &Settings::default()).unwrap();
    assert_defaults(&ok(&path).0);
}

#[test]
fn ac12_a_failed_save_leaves_no_temporary_file() {
    let dir = TempDir::new("s-failsave");
    // The target is a non-empty folder: the rename cannot replace it.
    let path = dir.path().join("settings.json");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), "x").unwrap();
    assert!(save(&path, &Settings::default()).is_err());
    let stray: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "settings.json")
        .collect();
    assert!(stray.is_empty(), "left behind: {stray:?}");
}

#[test]
fn ac16_save_writes_only_the_six_fields() {
    let dir = TempDir::new("s-fields");
    let path = dir.path().join("settings.json");
    save(&path, &Settings::default()).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    let mut want = vec![
        "enabled",
        "pack_id",
        "volume",
        "pitch_variation",
        "key_up_sounds",
        "launch_at_login",
    ];
    want.sort();
    assert_eq!(keys, want);
}
