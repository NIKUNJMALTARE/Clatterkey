//! Tests for the pack loader and the decoder (feature 04: AC1, AC8 to AC12).
//! Valid packs come from `tests/fixtures/packs/`; invalid ones are built at
//! run time in per-test temporary folders.
//!
//! Signatures assumed from the plan:
//!
//! ```ignore
//! packs::load(dir: &Path) -> Result<Pack, PackError>
//! Pack { pub id: String, pub name: String, pub author: Option<String>,
//!        pub license: String, pub sounds: SoundSet }
//! PackError: Display (the spec's one-line message)
//! audio::decode(bytes: Vec<u8>, max: Duration) -> Result<Sample, DecodeError>
//! DecodeError::{Unrecognised, Corrupt(String), TooLong, Empty, Silent, ChangesFormat}
//! ```
//!
//! `packs::paths::resolve` is not assumed public, so the path rules (AC10)
//! are tested through `load`. Errors are matched by their `Display` text
//! (pack id, field, file), not by `PackErrorKind` variant names, which the
//! plan does not fix.

use std::fs;
use std::time::Duration;

use mechanical_key_sounds_lib::audio::{DecodeError, Sample, decode};
use mechanical_key_sounds_lib::keys::{KeyGroup, KeyKind};
use mechanical_key_sounds_lib::packs::{PackError, PackErrorKind, load};

mod common;
use common::*;

fn load_err(dir: &std::path::Path) -> String {
    match load(dir) {
        Ok(_) => panic!("expected the pack to be rejected: {}", dir.display()),
        Err(e) => e.to_string(),
    }
}

fn load_error(dir: &std::path::Path) -> PackError {
    match load(dir) {
        Ok(_) => panic!("expected the pack to be rejected: {}", dir.display()),
        Err(e) => e,
    }
}

fn peak(sample: &Sample) -> f32 {
    sample.samples().iter().fold(0.0, |m, s| m.max(s.abs()))
}

fn assert_names_pack(msg: &str, id: &str) {
    assert!(
        msg.starts_with(&format!("pack \"{id}\"")),
        "message does not start with the pack: {msg}"
    );
}

// ---- AC1: metadata and samples in memory ----

#[test]
fn ac1_a_valid_pack_reports_id_name_author_and_license() {
    let pack = load(&fixture_pack("full")).expect("full pack loads");
    assert_eq!(pack.id, "full");
    assert_eq!(pack.name, "Full test pack");
    assert_eq!(pack.author.as_deref(), Some("Test Author"));
    assert_eq!(pack.license, "CC0-1.0");
}

#[test]
fn ac1_author_is_optional() {
    let pack = load(&fixture_pack("default-only")).unwrap();
    assert_eq!(pack.author, None);
    assert_eq!(pack.name, "Default only");
}

#[test]
fn ac1_every_group_has_its_samples_in_memory() {
    let pack = load(&fixture_pack("full")).unwrap();
    let expect_down = [
        (KeyGroup::Space, 70),
        (KeyGroup::Enter, 90),
        (KeyGroup::Backspace, 110),
        (KeyGroup::Modifier, 130),
    ];
    for (group, len) in expect_down {
        let sample = pack.sounds.pick(KeyKind::Down, group).unwrap();
        assert_eq!(ms(sample), len, "{}", group.name());
        assert!(peak(sample) > 0.01, "{} is silent", group.name());
    }
    let default = pack.sounds.pick(KeyKind::Down, KeyGroup::Default).unwrap();
    assert!((40..=50).contains(&ms(default)));
    assert_eq!(
        ms(pack.sounds.pick(KeyKind::Up, KeyGroup::Default).unwrap()),
        150
    );
    assert_eq!(
        ms(pack.sounds.pick(KeyKind::Up, KeyGroup::Space).unwrap()),
        170
    );
}

#[test]
fn ac1_the_id_is_the_folder_name_even_with_a_trailing_slash() {
    let with_slash = format!("{}/", fixture_pack("full").display());
    let pack = load(std::path::Path::new(&with_slash)).unwrap();
    assert_eq!(pack.id, "full");
}

// ---- AC8: formats, channels, rates ----

/// (file, channels, rate). There is no mono OGG fixture: ffmpeg's Vorbis
/// encoder is stereo only (see scripts/make-test-fixtures.sh).
const FORMATS: [(&str, u16, u32); 15] = [
    ("tone-mono-44100.wav", 1, 44100),
    ("tone-mono-48000.wav", 1, 48000),
    ("tone-stereo-44100.wav", 2, 44100),
    ("tone-stereo-48000.wav", 2, 48000),
    ("tone-mono-44100.flac", 1, 44100),
    ("tone-mono-48000.flac", 1, 48000),
    ("tone-stereo-44100.flac", 2, 44100),
    ("tone-stereo-48000.flac", 2, 48000),
    ("tone-mono-44100.mp3", 1, 44100),
    ("tone-mono-48000.mp3", 1, 48000),
    ("tone-stereo-44100.mp3", 2, 44100),
    ("tone-stereo-48000.mp3", 2, 48000),
    ("tone-stereo-44100.ogg", 2, 44100),
    ("tone-stereo-48000.ogg", 2, 48000),
    ("wav-named-ogg.ogg", 1, 44100),
];

fn assert_200ms(sample: &Sample, what: &str) {
    let ms = sample.duration().as_secs_f64() * 1000.0;
    assert!((ms - 200.0).abs() <= 5.0, "{what}: {ms} ms, want 200 +-5");
    assert!(peak(sample) > 0.01, "{what} is silent");
}

#[test]
fn ac8_decode_reads_every_format_channel_count_and_rate() {
    for (file, channels, rate) in FORMATS {
        let bytes = fs::read(fixture_audio(file)).unwrap();
        let sample =
            decode(bytes, Duration::from_secs(3)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(sample.channels(), channels, "{file}");
        assert_eq!(sample.sample_rate(), rate, "{file}");
        assert_200ms(&sample, file);
    }
}

#[test]
fn ac8_each_format_loads_through_a_pack_with_the_right_length() {
    for (file, _, _) in FORMATS {
        let pack = TestPack::new("one", &json_default(&["audio/x.bin"]));
        pack.copy_fixture("audio/x.bin", file);
        let loaded = load(&pack.dir).unwrap_or_else(|e| panic!("{file}: {e}"));
        let sample = loaded
            .sounds
            .pick(KeyKind::Down, KeyGroup::Default)
            .unwrap();
        assert_200ms(sample, file);
    }
}

#[test]
fn ac8_a_pack_using_every_format_at_once_loads() {
    let pack = load(&fixture_pack("formats")).expect("formats pack loads");
    assert_eq!(pack.id, "formats");
    assert!(pack.sounds.pick(KeyKind::Down, KeyGroup::Default).is_some());
}

#[test]
fn ac8_a_wav_named_ogg_still_loads() {
    let bytes = fs::read(fixture_audio("wav-named-ogg.ogg")).unwrap();
    assert!(decode(bytes, Duration::from_secs(3)).is_ok());
    let pack = TestPack::new("renamed", &json_default(&["sounds/click.ogg"]));
    pack.copy_fixture("sounds/click.ogg", "wav-named-ogg.ogg");
    assert!(load(&pack.dir).is_ok());
}

#[test]
fn ac8_the_extension_never_decides_the_format() {
    // An MP3 named .wav and a FLAC named .mp3.
    let pack = TestPack::new("liar", &json_default(&["a.wav", "b.mp3"]));
    pack.copy_fixture("a.wav", "tone-mono-44100.mp3");
    pack.copy_fixture("b.mp3", "tone-stereo-48000.flac");
    assert!(load(&pack.dir).is_ok());
}

// ---- decoder errors (AC9, at decode level) ----

#[test]
fn ac9_decode_rejects_silence() {
    let bytes = fs::read(fixture_audio("silent.wav")).unwrap();
    assert!(matches!(
        decode(bytes, Duration::from_secs(3)),
        Err(DecodeError::Silent)
    ));
}

#[test]
fn ac9_decode_rejects_an_empty_sample() {
    let bytes = wav_bytes(44100, 0, 1000);
    assert!(matches!(
        decode(bytes, Duration::from_secs(3)),
        Err(DecodeError::Empty)
    ));
}

#[test]
fn ac9_decode_rejects_a_sample_longer_than_the_limit() {
    let bytes = fs::read(fixture_audio("long-4s.wav")).unwrap();
    assert!(matches!(
        decode(bytes, Duration::from_secs(3)),
        Err(DecodeError::TooLong)
    ));
}

#[test]
fn ac9_decode_accepts_the_same_file_when_the_limit_is_higher() {
    let bytes = fs::read(fixture_audio("long-4s.wav")).unwrap();
    assert!(decode(bytes, Duration::from_secs(5)).is_ok());
}

#[test]
fn ac9_decode_rejects_a_file_that_is_not_audio() {
    let bytes = fs::read(fixture_audio("not-audio.txt")).unwrap();
    assert!(matches!(
        decode(bytes, Duration::from_secs(3)),
        Err(DecodeError::Unrecognised)
    ));
}

#[test]
fn ac9_decode_errors_have_a_message() {
    let bytes = fs::read(fixture_audio("silent.wav")).unwrap();
    let e = decode(bytes, Duration::from_secs(3)).err().unwrap();
    assert!(!e.to_string().is_empty());
}

// ---- AC9: every invalid case, one test each ----

#[test]
fn ac9_message_has_the_spec_shape_for_a_missing_file() {
    let pack = TestPack::new(
        "blue",
        r#"{"name":"B","license":"x","keys":{"default":["down/a.wav"],"space":["down/space.wav"]}}"#,
    );
    pack.copy_fixture("down/a.wav", "tone-mono-44100.wav");
    assert_eq!(
        load_err(&pack.dir),
        r#"pack "blue": keys.space[0] "down/space.wav": file not found"#
    );
}

#[test]
fn ac9_a_missing_folder_is_rejected_naming_the_pack() {
    let tmp = TempDir::new("nofolder");
    let msg = load_err(&tmp.path().join("ghost"));
    assert_names_pack(&msg, "ghost");
}

#[test]
fn ac9_a_missing_pack_json_is_rejected() {
    let tmp = TempDir::new("nojson");
    let dir = tmp.path().join("empty");
    fs::create_dir_all(&dir).unwrap();
    let msg = load_err(&dir);
    assert_names_pack(&msg, "empty");
    assert!(msg.contains("pack.json"), "{msg}");
}

#[test]
fn ac9_json_that_is_not_json_is_rejected() {
    let pack = TestPack::new("broken", "{ this is not json");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "broken");
    assert!(msg.contains("pack.json"), "{msg}");
}

#[test]
fn ac9_a_pack_json_over_64_kib_is_rejected_even_if_otherwise_valid() {
    let padding = "x".repeat(70 * 1024);
    let json =
        format!(r#"{{"name":"T","license":"x","pad":"{padding}","keys":{{"default":["a.wav"]}}}}"#);
    let pack = TestPack::new("big", &json);
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "big");
}

#[test]
fn ac9_required_fields_missing_or_of_the_wrong_type_are_rejected() {
    use PackErrorKind::*;
    let cases: Vec<(&str, &str, &str, PackErrorKind)> = vec![
        (
            "p1",
            r#"{"license":"x","keys":{"default":["a.wav"]}}"#,
            "name",
            Missing,
        ),
        (
            "p2",
            r#"{"name":"T","keys":{"default":["a.wav"]}}"#,
            "license",
            Missing,
        ),
        ("p3", r#"{"name":"T","license":"x"}"#, "keys", Missing),
        (
            "p4",
            r#"{"name":"","license":"x","keys":{"default":["a.wav"]}}"#,
            "name",
            NotText,
        ),
        (
            "p5",
            r#"{"name":"   ","license":"x","keys":{"default":["a.wav"]}}"#,
            "name",
            NotText,
        ),
        (
            "p6",
            r#"{"name":"T","license":"","keys":{"default":["a.wav"]}}"#,
            "license",
            NotText,
        ),
        (
            "p7",
            r#"{"name":5,"license":"x","keys":{"default":["a.wav"]}}"#,
            "name",
            NotText,
        ),
        (
            "p8",
            r#"{"name":"T","license":"x","keys":["a.wav"]}"#,
            "keys",
            NotAnObject,
        ),
        (
            "p9",
            r#"{"name":"T","license":"x","keys":{"default":"a.wav"}}"#,
            "keys.default",
            NotAList,
        ),
        (
            "p10",
            r#"{"name":"T","license":"x","keys":{"default":[5]}}"#,
            "keys.default[0]",
            NotAFileName,
        ),
        (
            "p11",
            r#"{"name":"T","license":"x","author":3,"keys":{"default":["a.wav"]}}"#,
            "author",
            NotText,
        ),
        (
            "p12",
            r#"{"name":"T","license":"x","keys":{"default":["a.wav"]},"up":["a.wav"]}"#,
            "up",
            NotAnObject,
        ),
    ];
    for (id, json, field, kind) in cases {
        let pack = TestPack::new(id, json);
        pack.copy_fixture("a.wav", "tone-mono-44100.wav");
        let e = load_error(&pack.dir);
        assert_eq!(e.field.as_deref(), Some(field), "{id}: {e}");
        assert_eq!(e.kind, kind, "{id}: {e}");
        assert_names_pack(&e.to_string(), id);
    }
}

#[test]
fn ac9_keys_without_a_default_list_is_rejected() {
    let pack = TestPack::new(
        "nodefault",
        r#"{"name":"T","license":"x","keys":{"space":["a.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let e = load_error(&pack.dir);
    assert_eq!(e.field.as_deref(), Some("keys.default"));
    assert_eq!(e.kind, PackErrorKind::Missing);
    assert_names_pack(&e.to_string(), "nodefault");
}

#[test]
fn ac9_an_empty_default_list_is_rejected() {
    let pack = TestPack::new(
        "emptydefault",
        r#"{"name":"T","license":"x","keys":{"default":[],"space":["a.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let e = load_error(&pack.dir);
    assert_eq!(e.field.as_deref(), Some("keys.default"));
    assert_eq!(e.kind, PackErrorKind::EmptyDefault);
    assert_names_pack(&e.to_string(), "emptydefault");
}

#[test]
fn ac9_an_empty_optional_down_group_uses_default() {
    let pack = TestPack::new(
        "emptyspace",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"],"space":[]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let loaded = load(&pack.dir).unwrap();
    assert!(loaded.sounds.pick(KeyKind::Down, KeyGroup::Space).is_some());
}

#[test]
fn ac9_a_missing_file_in_an_up_group_names_the_up_field() {
    let pack = TestPack::new(
        "upmissing",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"]},"up":{"enter":["up/gone.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    assert_eq!(
        load_err(&pack.dir),
        r#"pack "upmissing": up.enter[0] "up/gone.wav": file not found"#
    );
}

#[test]
fn ac9_a_file_that_cannot_be_read_is_rejected() {
    // A folder where a file should be: it exists but cannot be read as a file.
    let pack = TestPack::new("unreadable", &json_default(&["down/a.wav"]));
    fs::create_dir_all(pack.dir.join("down/a.wav")).unwrap();
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "unreadable");
    assert!(msg.contains("down/a.wav"), "{msg}");
}

#[test]
fn ac9_a_file_over_10_mb_is_rejected() {
    // 11 MiB is over the limit whether "10 MB" means 10^7 or 2^20 * 10.
    let pack = TestPack::new("huge", &json_default(&["a.wav"]));
    pack.add("a.wav", &vec![0u8; 11 * 1024 * 1024]);
    let e = load_error(&pack.dir);
    assert_eq!(e.kind, PackErrorKind::FileTooLarge);
    assert_eq!(e.file.as_deref(), Some("a.wav"));
    assert_eq!(e.field.as_deref(), Some("keys.default[0]"));
    assert_names_pack(&e.to_string(), "huge");
}

#[test]
fn ac9_a_file_that_is_not_audio_is_rejected() {
    let pack = TestPack::new("text", &json_default(&["notes.wav"]));
    pack.copy_fixture("notes.wav", "not-audio.txt");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "text");
    assert!(
        msg.contains("keys.default[0]") && msg.contains("notes.wav"),
        "{msg}"
    );
}

#[test]
fn ac9_a_file_that_cannot_be_decoded_is_rejected() {
    let bytes = fs::read(fixture_audio("tone-mono-44100.flac")).unwrap();
    let pack = TestPack::new("cut", &json_default(&["a.flac"]));
    pack.add("a.flac", &bytes[..bytes.len().min(60)]);
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "cut");
    assert!(msg.contains("a.flac"), "{msg}");
}

#[test]
fn ac9_a_silent_sample_is_rejected() {
    let pack = TestPack::new("quiet", &json_default(&["a.wav"]));
    pack.copy_fixture("a.wav", "silent.wav");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "quiet");
    assert!(msg.contains("a.wav"), "{msg}");
}

#[test]
fn ac9_an_empty_sample_is_rejected() {
    let pack = TestPack::new("nothing", &json_default(&["a.wav"]));
    pack.add("a.wav", &wav_bytes(44100, 0, 1000));
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "nothing");
    assert!(msg.contains("a.wav"), "{msg}");
}

#[test]
fn ac9_a_sample_longer_than_3_seconds_is_rejected() {
    let pack = TestPack::new("long", &json_default(&["a.wav"]));
    pack.copy_fixture("a.wav", "long-4s.wav");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "long");
    assert!(msg.contains("a.wav"), "{msg}");
}

fn many_files(id: &str, n: usize, wav: &[u8]) -> TestPack {
    let names: Vec<String> = (0..n).map(|i| format!("s/{i:03}.wav")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let pack = TestPack::new(id, &json_default(&refs));
    for name in &names {
        pack.add(name, wav);
    }
    pack
}

#[test]
fn ac9_exactly_100_distinct_files_load() {
    let pack = many_files("hundred", 100, &wav_bytes(8000, 100, 1000));
    assert!(load(&pack.dir).is_ok());
}

#[test]
fn ac9_more_than_100_distinct_files_are_rejected() {
    let pack = many_files("many", 101, &wav_bytes(8000, 100, 1000));
    assert_names_pack(&load_err(&pack.dir), "many");
}

#[test]
fn ac9_a_file_listed_many_times_counts_once_towards_the_limit() {
    let names = vec!["a.wav"; 150];
    let pack = TestPack::new("repeat", &json_default(&names));
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    assert!(load(&pack.dir).is_ok());
}

#[test]
fn ac9_more_than_60_seconds_of_audio_in_total_is_rejected() {
    // 22 distinct files of 2.9 s each, every one under the 3 s limit.
    let pack = many_files("hour", 22, &wav_bytes(4000, 2900, 1000));
    assert_names_pack(&load_err(&pack.dir), "hour");
}

#[test]
fn ac9_just_under_60_seconds_in_total_loads() {
    // 20 files of 2.9 s is 58 s.
    let pack = many_files("minute", 20, &wav_bytes(4000, 2900, 1000));
    assert!(load(&pack.dir).is_ok());
}

#[test]
fn ac9_one_bad_file_rejects_the_whole_pack() {
    let pack = TestPack::new(
        "half",
        r#"{"name":"T","license":"x","keys":{"default":["good.wav"],"space":["bad.wav"]},"up":{"default":["good.wav"]}}"#,
    );
    pack.copy_fixture("good.wav", "tone-mono-44100.wav");
    pack.copy_fixture("bad.wav", "silent.wav");
    // The result type holds either a whole pack or an error, nothing between.
    assert!(load(&pack.dir).is_err());
}

#[test]
fn ac9_errors_carry_no_key_codes_or_typed_text() {
    let pack = TestPack::new("clean", &json_default(&["gone.wav"]));
    let msg = load_err(&pack.dir);
    for word in ["KeyA", "Unknown(", "Space,", "Backspace,"] {
        assert!(!msg.contains(word), "{msg}");
    }
}

// ---- AC10: file names that leave the folder ----

fn one_bad_name(id: &str, name: &str) -> TestPack {
    // The outside file is a valid sound, so only the path rule can reject it.
    let json = json_default(&[name]);
    let pack = TestPack::new(id, &json);
    fs::write(
        pack.tmp.path().join("outside.wav"),
        wav_bytes(44100, 100, 1000),
    )
    .unwrap();
    pack
}

#[test]
fn ac10_dot_dot_is_rejected_and_the_outside_file_is_not_used() {
    let pack = one_bad_name("dots", "../outside.wav");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "dots");
    assert!(msg.contains("../outside.wav"), "{msg}");
}

#[test]
fn ac10_dot_dot_in_the_middle_is_rejected() {
    let pack = one_bad_name("middle", "down/../../outside.wav");
    pack.add("down/x.wav", &wav_bytes(44100, 100, 1000));
    assert_names_pack(&load_err(&pack.dir), "middle");
}

#[test]
fn ac10_an_absolute_unix_path_is_rejected() {
    let pack = one_bad_name("abs", "/etc/x.wav");
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "abs");
    assert!(msg.contains("/etc/x.wav"), "{msg}");
}

#[test]
fn ac10_an_absolute_path_to_a_real_file_is_rejected() {
    let pack = TestPack::new("real", "{}");
    let target = pack.tmp.path().join("outside.wav");
    fs::write(&target, wav_bytes(44100, 100, 1000)).unwrap();
    let json = json_default(&[target.to_str().unwrap()]);
    fs::write(pack.dir.join("pack.json"), json).unwrap();
    assert_names_pack(&load_err(&pack.dir), "real");
}

#[test]
fn ac10_a_windows_drive_path_is_rejected_on_every_os() {
    let pack = one_bad_name("drive", r"C:\x.wav");
    assert_names_pack(&load_err(&pack.dir), "drive");
}

#[test]
fn ac10_a_backslash_is_rejected_on_every_os() {
    let pack = one_bad_name("slashes", r"down\a.wav");
    assert_names_pack(&load_err(&pack.dir), "slashes");
}

#[test]
fn ac10_an_empty_name_is_rejected() {
    let pack = TestPack::new("empty", &json_default(&[""]));
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    assert_names_pack(&load_err(&pack.dir), "empty");
}

#[cfg(unix)]
#[test]
fn ac10_a_link_to_a_file_outside_the_folder_is_rejected() {
    let pack = one_bad_name("link", "down/a.wav");
    fs::create_dir_all(pack.dir.join("down")).unwrap();
    std::os::unix::fs::symlink(
        pack.tmp.path().join("outside.wav"),
        pack.dir.join("down/a.wav"),
    )
    .unwrap();
    let msg = load_err(&pack.dir);
    assert_names_pack(&msg, "link");
    assert!(msg.contains("down/a.wav"), "{msg}");
}

#[cfg(unix)]
#[test]
fn ac10_a_link_to_a_folder_outside_is_rejected() {
    let pack = one_bad_name("dirlink", "linked/outside.wav");
    let elsewhere = pack.tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("outside.wav"), wav_bytes(44100, 100, 1000)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, pack.dir.join("linked")).unwrap();
    assert_names_pack(&load_err(&pack.dir), "dirlink");
}

#[cfg(unix)]
#[test]
fn ac10_a_link_that_stays_inside_the_folder_is_fine() {
    let pack = TestPack::new("inside", &json_default(&["alias.wav"]));
    pack.copy_fixture("real.wav", "tone-mono-44100.wav");
    std::os::unix::fs::symlink(pack.dir.join("real.wav"), pack.dir.join("alias.wav")).unwrap();
    assert!(load(&pack.dir).is_ok());
}

// ---- AC11: one decode, shared buffer ----

#[test]
fn ac11_a_file_in_two_groups_shares_one_buffer() {
    let pack = load(&fixture_pack("shared")).unwrap();
    let space = pack
        .sounds
        .pick(KeyKind::Down, KeyGroup::Space)
        .unwrap()
        .clone();
    let shared_with_default = (0..100).any(|_| {
        pack.sounds
            .pick(KeyKind::Down, KeyGroup::Default)
            .unwrap()
            .same_buffer(&space)
    });
    assert!(
        shared_with_default,
        "space does not share a buffer with default"
    );
}

#[test]
fn ac11_a_file_in_down_and_up_shares_one_buffer() {
    let pack = TestPack::new(
        "downup",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"]},"up":{"default":["a.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let loaded = load(&pack.dir).unwrap();
    let d = loaded
        .sounds
        .pick(KeyKind::Down, KeyGroup::Default)
        .unwrap();
    let u = loaded.sounds.pick(KeyKind::Up, KeyGroup::Default).unwrap();
    assert!(d.same_buffer(u));
}

#[test]
fn ac11_two_different_files_do_not_share_a_buffer() {
    let pack = TestPack::new(
        "twofiles",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"],"space":["b.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    pack.copy_fixture("b.wav", "tone-mono-44100.wav"); // equal bytes, other file
    let loaded = load(&pack.dir).unwrap();
    let d = loaded
        .sounds
        .pick(KeyKind::Down, KeyGroup::Default)
        .unwrap();
    let s = loaded.sounds.pick(KeyKind::Down, KeyGroup::Space).unwrap();
    assert!(!d.same_buffer(s));
}

#[cfg(unix)]
#[test]
fn ac11_two_names_for_one_file_are_decoded_once() {
    // The plan counts distinct canonical paths, so a link and its target are one.
    let pack = TestPack::new(
        "alias",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"],"space":["b.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    std::os::unix::fs::symlink(pack.dir.join("a.wav"), pack.dir.join("b.wav")).unwrap();
    let loaded = load(&pack.dir).unwrap();
    let d = loaded
        .sounds
        .pick(KeyKind::Down, KeyGroup::Default)
        .unwrap();
    let s = loaded.sounds.pick(KeyKind::Down, KeyGroup::Space).unwrap();
    assert!(d.same_buffer(s));
}

// ---- AC12: unknown fields and group names ----

#[test]
fn ac12_an_extra_top_level_field_is_ignored() {
    let pack = load(&fixture_pack("extra-field")).expect("extra field loads");
    assert_eq!(pack.name, "Extra field");
}

#[test]
fn ac12_an_unknown_group_in_keys_is_rejected_naming_it() {
    for bad in ["Space", "tab", "arrows"] {
        let json = format!(
            r#"{{"name":"T","license":"x","keys":{{"default":["a.wav"],"{bad}":["a.wav"]}}}}"#
        );
        let pack = TestPack::new("typo", &json);
        pack.copy_fixture("a.wav", "tone-mono-44100.wav");
        let e = load_error(&pack.dir);
        assert_eq!(e.kind, PackErrorKind::UnknownGroup);
        assert_eq!(e.field, Some(format!("keys.{bad}")));
        assert_names_pack(&e.to_string(), "typo");
    }
}

#[test]
fn ac12_an_unknown_group_in_up_is_rejected_naming_it() {
    let pack = TestPack::new(
        "uptypo",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"]},"up":{"Enter":["a.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let e = load_error(&pack.dir);
    assert_eq!(e.kind, PackErrorKind::UnknownGroup);
    assert_eq!(e.field.as_deref(), Some("up.Enter"));
    assert_names_pack(&e.to_string(), "uptypo");
}

#[test]
fn ac12_an_unknown_group_is_rejected_even_when_its_files_are_fine() {
    // The typo must not be silently ignored, and must not need a missing file.
    let pack = TestPack::new(
        "fine",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"],"Space":["a.wav"]}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    assert!(load(&pack.dir).is_err());
}

// ---- Edge cases from the plan ----

#[test]
fn a_pack_json_with_a_byte_order_mark_loads() {
    let json = format!("\u{feff}{}", json_default(&["a.wav"]));
    let pack = TestPack::new("bom", &json);
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let loaded = load(&pack.dir).expect("a BOM is accepted");
    assert_eq!(loaded.id, "bom");
}

#[test]
fn file_names_with_spaces_and_non_ascii_letters_load() {
    let pack = TestPack::new("sp\u{e4}ce", &json_default(&["down/my click \u{e9}.wav"]));
    pack.copy_fixture("down/my click \u{e9}.wav", "tone-mono-44100.wav");
    let loaded = load(&pack.dir).unwrap();
    assert_eq!(loaded.id, "sp\u{e4}ce");
}

#[test]
fn an_empty_up_object_means_up_is_silent() {
    let pack = TestPack::new(
        "emptyup",
        r#"{"name":"T","license":"x","keys":{"default":["a.wav"]},"up":{}}"#,
    );
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    let loaded = load(&pack.dir).unwrap();
    for group in KeyGroup::ALL {
        assert!(loaded.sounds.pick(KeyKind::Up, group).is_none());
    }
}

#[test]
fn a_group_listing_the_same_file_twice_loads() {
    let pack = TestPack::new("twice", &json_default(&["a.wav", "a.wav"]));
    pack.copy_fixture("a.wav", "tone-mono-44100.wav");
    assert!(load(&pack.dir).is_ok());
}

#[test]
fn files_may_sit_in_subfolders() {
    let pack = TestPack::new("deep", &json_default(&["a/b/c/d.wav"]));
    pack.copy_fixture("a/b/c/d.wav", "tone-mono-44100.wav");
    assert!(load(&pack.dir).is_ok());
}

// ---- Channels and corrupt float data ----

/// A WAV with `channels` channels of 16-bit PCM, 100 ms at 8 kHz.
fn wav_channels(channels: u16) -> Vec<u8> {
    let rate = 8000u32;
    let frames = 800u32;
    let data_len = frames * u32::from(channels) * 2;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..(frames * u32::from(channels)) {
        let v: i16 = if i % 2 == 0 { 1000 } else { -1000 };
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A mono 32-bit float WAV (format tag 3) holding `values`.
fn wav_float(values: &[f32]) -> Vec<u8> {
    let rate = 8000u32;
    let data_len = (values.len() * 4) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[test]
fn ac9_decode_rejects_three_or_more_channels() {
    for channels in [3u16, 4, 6] {
        let r = decode(wav_channels(channels), Duration::from_secs(3));
        assert!(
            matches!(r, Err(DecodeError::TooManyChannels)),
            "{channels} channels"
        );
    }
}

#[test]
fn ac9_decode_still_accepts_two_channels_from_the_same_builder() {
    assert!(decode(wav_channels(2), Duration::from_secs(3)).is_ok());
}

#[test]
fn ac9_a_pack_file_with_three_channels_is_rejected_as_too_many_channels() {
    let pack = TestPack::new("surround", &json_default(&["a.wav"]));
    pack.add("a.wav", &wav_channels(3));
    let e = load_error(&pack.dir);
    assert_eq!(e.kind, PackErrorKind::Audio(DecodeError::TooManyChannels));
    assert_eq!(e.file.as_deref(), Some("a.wav"));
}

#[test]
fn ac9_a_float_wav_with_nan_is_rejected_as_corrupt() {
    let mut values = vec![0.5f32; 400];
    values[200] = f32::NAN;
    let r = decode(wav_float(&values), Duration::from_secs(3));
    assert!(
        matches!(r, Err(DecodeError::Corrupt(_))),
        "got ok or other error"
    );
}

#[test]
fn ac9_a_clean_float_wav_decodes() {
    let values = vec![0.5f32; 400];
    assert!(decode(wav_float(&values), Duration::from_secs(3)).is_ok());
}
