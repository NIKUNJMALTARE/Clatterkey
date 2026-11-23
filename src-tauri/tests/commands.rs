//! Tests for the settings window's core side (feature 06: AC14, AC15, AC16,
//! AC19 core half, AC20, AC21, AC22), over temporary settings files and
//! fixture packs. See `support/mod.rs` for the interfaces assumed.
//!
//! The fixture pack `full` is named "Full test pack" (author "Test Author"),
//! `plain` is "Default only" (no author); `full` plays space at 70 ms and
//! letters at 40 to 50 ms.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use mechanical_key_sounds_lib::audio::{Live, OutputStatus, SoundSet, builtin_click};
use mechanical_key_sounds_lib::commands::{Core, CoreParts};
use mechanical_key_sounds_lib::keys::KeyCode;
use mechanical_key_sounds_lib::packs::Library;
use mechanical_key_sounds_lib::permission::{Permission, PermissionState};
use mechanical_key_sounds_lib::settings::{self, LoadProblem, Settings};
use mechanical_key_sounds_lib::startup::choose_pack;
use serde_json::{Value, json};

mod common;
use common::*;
mod support;
use support::*;

const PACKS: [(&str, &str); 2] = [("full", "full"), ("plain", "default-only")];

fn bundled() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("packs")
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

// AC14: listing packs

#[test]
fn ac14_the_bundled_packs_are_listed_sorted_by_name_with_author_and_licence() {
    let rig = rig(Some(&file("brown")), &[]);
    // A core over the real bundled folder.
    let (_, err) = fake_err();
    let loaded = settings::load_or_create_report(&rig.settings_path, &err);
    let chosen = choose_pack(Some(&bundled()), "brown", &err);
    let core = Core::new(CoreParts {
        settings_path: Some(rig.settings_path.clone()),
        library: Library::bundled_only(bundled()),
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::new(Live::new(chosen.sounds)),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    });
    let list = core.list_packs();
    let rows = list.as_array().expect("an array");
    let names: Vec<&str> = rows.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "Blue switches",
            "Brown switches",
            "Red switches",
            "Typewriter"
        ]
    );
    let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["blue", "brown", "red", "typewriter"]);
    for row in rows {
        assert_eq!(row["license"], "CC0-1.0");
        assert_eq!(row["author"], "Mechanical Key Sounds contributors");
    }
}

#[test]
fn ac14_a_pack_without_an_author_has_a_null_author() {
    let rig = rig(Some(&file("full")), &PACKS);
    let list = rig.core.list_packs();
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    // Sorted by name: "Default only" before "Full test pack".
    assert_eq!(rows[0]["id"], "plain");
    assert_eq!(rows[0]["name"], "Default only");
    assert!(rows[0]["author"].is_null());
    assert_eq!(rows[0]["license"], "CC0-1.0");
    assert_eq!(rows[1]["id"], "full");
    assert_eq!(rows[1]["author"], "Test Author");
}

#[test]
fn ac14_folders_without_a_readable_pack_json_and_stray_files_are_left_out() {
    let rig = rig_with(Some(&file("full")), &PACKS, |packs| {
        fs::create_dir_all(packs.join("empty")).unwrap();
        fs::create_dir_all(packs.join("garbled")).unwrap();
        fs::write(packs.join("garbled").join("pack.json"), "{ not json").unwrap();
        fs::write(packs.join("stray.txt"), "hello").unwrap();
        // A readable pack in a folder whose name cannot be a pack id.
        copy_dir(&fixture_pack("full"), &packs.join("has space"));
    });
    let list = rig.core.list_packs();
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["plain", "full"]);
}

#[test]
fn ac14_with_no_packs_folder_or_an_empty_one_the_list_is_empty() {
    let rig = rig(Some(&file("full")), &[]);
    assert_eq!(rig.core.list_packs(), json!([]));

    let (_, err) = fake_err();
    let loaded = settings::load_or_create_report(&rig.settings_path, &err);
    let core = Core::new(CoreParts {
        settings_path: None,
        library: Library::none(),
        loaded,
        playing: None,
        fell_back: true,
        live: Arc::new(Live::new(SoundSet::single(builtin_click()))),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    });
    assert_eq!(core.list_packs(), json!([]));
}

// get_settings and audio_status

#[test]
fn get_settings_carries_the_six_fields_the_playing_pack_and_no_problems() {
    let rig = rig(Some(&file("full")), &PACKS);
    let v = rig.core.get_settings();
    assert_eq!(v["settings"]["enabled"], true);
    assert_eq!(v["settings"]["pack_id"], "full");
    assert_eq!(v["settings"]["volume"], 0.8);
    assert_eq!(v["settings"]["pitch_variation"], 0.2);
    assert_eq!(v["settings"]["key_up_sounds"], false);
    assert_eq!(v["settings"]["launch_at_login"], true);
    assert_eq!(v["playing_pack"], "full");
    assert_eq!(v["startup"]["settings_unreadable"], false);
    assert_eq!(v["startup"]["replaced_fields"], json!([]));
    assert!(v["startup"]["unusable_pack"].is_null());
    assert_eq!(v["audio"], "playing");
}

#[test]
fn get_settings_reports_the_output_status_as_it_is_now() {
    use std::sync::atomic::{AtomicU8, Ordering};
    let status = Arc::new(AtomicU8::new(0));
    let rig = rig(Some(&file("full")), &PACKS);
    let (_, err) = fake_err();
    let loaded = settings::load_or_create_report(&rig.settings_path, &err);
    let chosen = choose_pack(Some(&rig.packs_path), "full", &err);
    let s = Arc::clone(&status);
    let core = Core::new(CoreParts {
        settings_path: Some(rig.settings_path.clone()),
        library: Library::bundled_only(rig.packs_path.clone()),
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::new(Live::new(chosen.sounds)),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(move || match s.load(Ordering::SeqCst) {
            0 => OutputStatus::Playing,
            1 => OutputStatus::NoDevice,
            _ => OutputStatus::Failed,
        }),
    });
    assert_eq!(core.audio_status(), "playing");
    status.store(1, Ordering::SeqCst);
    assert_eq!(core.audio_status(), "no_device");
    assert_eq!(core.get_settings()["audio"], "no_device");
    status.store(2, Ordering::SeqCst);
    assert_eq!(core.audio_status(), "failed");
    assert_eq!(core.get_settings()["audio"], "failed");
}

#[test]
fn a_fallback_at_start_up_is_reported_with_the_pack_actually_playing() {
    // "brown" is not in this folder, so the fallback is the built-in click.
    let rig = rig(Some(&file("nope")), &PACKS);
    let v = rig.core.get_settings();
    assert_eq!(v["settings"]["pack_id"], "nope");
    assert!(v["playing_pack"].is_null());
    assert_eq!(v["startup"]["unusable_pack"], "nope");
}

#[test]
fn a_fallback_to_the_default_pack_names_it_as_playing() {
    let rig = rig(
        Some(&file("nope")),
        &[("brown", "full"), ("plain", "default-only")],
    );
    let v = rig.core.get_settings();
    assert_eq!(v["playing_pack"], "brown");
    assert_eq!(v["startup"]["unusable_pack"], "nope");
}

// AC15: saving one field

#[test]
fn ac15_a_change_replaces_only_its_field_and_the_file_stays_complete() {
    let rig = rig(Some(&file("full")), &PACKS);
    let result = rig.core.set_settings(&json!({"volume": 0.35})).unwrap();
    assert_eq!(result["saved"], true);
    assert!(result["save_error"].is_null());
    assert_eq!(result["settings"]["volume"], 0.35);
    assert_eq!(result["playing_pack"], "full");

    assert_eq!(
        rig.settings(),
        Settings {
            enabled: true,
            pack_id: "full".into(),
            volume: 0.35,
            pitch_variation: 0.2,
            key_up_sounds: false,
            launch_at_login: true,
        }
    );
}

#[test]
fn ac15_each_of_the_three_fields_can_be_changed_and_saved() {
    let rig = rig(Some(&file("full")), &PACKS);
    rig.core.set_settings(&json!({"enabled": false})).unwrap();
    assert!(!rig.settings().enabled);
    rig.core.set_settings(&json!({"pack_id": "plain"})).unwrap();
    assert_eq!(rig.settings().pack_id, "plain");
    rig.core
        .set_settings(&json!({"enabled": true, "volume": 0.5}))
        .unwrap();
    let s = rig.settings();
    assert!(s.enabled);
    assert_eq!(s.volume, 0.5);
    assert_eq!(s.pack_id, "plain");
    assert_eq!(s.pitch_variation, 0.2);
    assert!(s.launch_at_login);
}

#[test]
fn ac15_the_write_is_atomic_and_leaves_no_temporary_file() {
    let rig = rig(Some(&file("full")), &PACKS);
    for i in 0..20 {
        rig.core
            .set_settings(&json!({"volume": i as f64 / 20.0}))
            .unwrap();
        let text = fs::read_to_string(&rig.settings_path).unwrap();
        serde_json::from_str::<Value>(&text).expect("the file is whole JSON after each save");
    }
    let names: Vec<String> = fs::read_dir(rig.settings_path.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["settings.json"]);
}

#[test]
fn ac15_a_patch_that_changes_nothing_succeeds() {
    let rig = rig(Some(&file("full")), &PACKS);
    let result = rig.core.set_settings(&json!({})).unwrap();
    assert_eq!(result["settings"]["volume"], 0.8);
    let result = rig.core.set_settings(&json!({"enabled": true})).unwrap();
    assert_eq!(result["settings"]["enabled"], true);
}

#[test]
fn ac15_volume_exactly_zero_one_and_negative_zero_are_accepted() {
    let rig = rig(Some(&file("full")), &PACKS);
    for v in [0.0_f64, 1.0, -0.0] {
        rig.core
            .set_settings(&json!({ "volume": v }))
            .unwrap_or_else(|e| panic!("{v}: {e}"));
        assert_eq!(rig.settings().volume.abs(), v.abs());
    }
}

#[test]
fn ac15_two_threads_changing_settings_leave_the_file_equal_to_what_is_in_force() {
    let rig = rig(Some(&file("full")), &PACKS);
    let mut handles = Vec::new();
    for t in 0..4 {
        let core = Arc::clone(&rig.core);
        handles.push(std::thread::spawn(move || {
            for i in 0..25 {
                let volume = ((t * 25 + i) % 100) as f64 / 100.0;
                core.set_settings(&json!({ "volume": volume })).unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let in_force = rig.core.get_settings()["settings"]["volume"]
        .as_f64()
        .unwrap();
    assert_eq!(rig.settings().volume, in_force);
    assert!(approx(rig.live.gain(), (in_force * in_force) as f32));
}

#[test]
fn ac15_choosing_a_pack_after_a_start_up_fallback_writes_the_new_id() {
    let rig = rig(Some(&file("nope")), &PACKS);
    let before = rig.bytes().unwrap();
    assert!(rig.settings().pack_id == "nope");
    let result = rig.core.set_settings(&json!({"pack_id": "full"})).unwrap();
    assert_eq!(result["playing_pack"], "full");
    assert_ne!(rig.bytes().unwrap(), before);
    assert_eq!(rig.settings().pack_id, "full");
}

// AC16: rejected patches

fn assert_rejected(rig: &Rig, patch: Value) {
    let before = rig.bytes();
    let gain = rig.live.gain();
    let enabled = rig.live.enabled();
    let result = rig.core.set_settings(&patch);
    assert!(
        result.is_err(),
        "{patch} should be rejected, got {result:?}"
    );
    assert!(
        !result.unwrap_err().is_empty(),
        "{patch}: the error says why"
    );
    assert_eq!(rig.bytes(), before, "{patch}: nothing is written");
    assert_eq!(rig.live.gain(), gain, "{patch}: gain unchanged");
    assert_eq!(rig.live.enabled(), enabled, "{patch}: enabled unchanged");
    let v = rig.core.get_settings();
    assert_eq!(v["settings"]["volume"], 0.8, "{patch}: settings unchanged");
    assert_eq!(v["settings"]["pack_id"], "full");
    assert_eq!(v["playing_pack"], "full");
}

#[test]
fn ac16_volume_out_of_range_or_not_a_number_is_rejected() {
    let rig = rig(Some(&file("full")), &PACKS);
    for volume in [
        json!(1.01),
        json!(-0.01),
        json!(2),
        json!(-1),
        json!(1e300),
        json!("0.5"),
        json!("NaN"),
        json!(true),
        json!(null),
        json!([0.5]),
    ] {
        assert_rejected(&rig, json!({ "volume": volume }));
    }
}

#[test]
fn ac16_a_pack_id_that_is_not_a_listed_pack_is_rejected() {
    let rig = rig(Some(&file("full")), &PACKS);
    for id in [
        "", "../full", "C:full", "nope", "Full", "full/", "a b", "..", ".",
    ] {
        assert_rejected(&rig, json!({ "pack_id": id }));
    }
    assert_rejected(&rig, json!({"pack_id": 5}));
    assert_rejected(&rig, json!({"pack_id": null}));
}

#[test]
fn ac16_enabled_must_be_a_bool() {
    let rig = rig(Some(&file("full")), &PACKS);
    for v in [json!("false"), json!(0), json!(null)] {
        assert_rejected(&rig, json!({ "enabled": v }));
    }
}

#[test]
fn ac16_fields_other_features_own_are_rejected_by_name() {
    let rig = rig(Some(&file("full")), &PACKS);
    for field in ["launch_at_login", "bogus"] {
        assert_rejected(&rig, json!({ field: true }));
        let err = rig.core.set_settings(&json!({ field: true })).unwrap_err();
        assert!(err.contains(field), "{err:?} names {field}");
    }
}

#[test]
fn ac16_a_patch_that_is_not_an_object_is_rejected() {
    let rig = rig(Some(&file("full")), &PACKS);
    for patch in [
        json!(null),
        json!(5),
        json!("volume"),
        json!([{"volume": 0.5}]),
    ] {
        assert_rejected(&rig, patch);
    }
}

#[test]
fn ac16_one_invalid_field_rejects_the_whole_patch() {
    let rig = rig(Some(&file("full")), &PACKS);
    assert_rejected(&rig, json!({"enabled": false, "volume": 2.0}));
    assert_rejected(&rig, json!({"enabled": false, "pack_id": "nope"}));
    assert_rejected(&rig, json!({"volume": 0.1, "launch_at_login": true}));
}

// AC19 (core half): switching packs

#[test]
fn ac19_choosing_the_pack_already_playing_does_not_load_it_again() {
    // With the pack's audio gone a reload would fail; the id is still listed.
    let rig = rig(Some(&file("full")), &PACKS);
    fs::remove_dir_all(rig.packs_path.join("full").join("down")).unwrap();
    let result = rig
        .core
        .set_settings(&json!({"pack_id": "full"}))
        .expect("no reload");
    assert_eq!(result["playing_pack"], "full");
}

#[test]
fn ac19_a_pack_that_fails_to_load_changes_nothing_and_says_why() {
    let rig = rig_with(Some(&file("full")), &PACKS, |packs| {
        let dir = packs.join("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("pack.json"), json_default(&["down/missing.wav"])).unwrap();
    });
    let before = rig.bytes();
    let error = rig
        .core
        .set_settings(&json!({"pack_id": "broken"}))
        .unwrap_err();
    assert!(
        error.contains("missing.wav") || error.contains("broken"),
        "{error}"
    );
    assert_eq!(rig.bytes(), before);
    assert_eq!(rig.core.get_settings()["playing_pack"], "full");
    assert_eq!(rig.core.get_settings()["settings"]["pack_id"], "full");
    // Still playing the old pack: a space is 70 ms in `full`.
    rig.player.handle(down(KeyCode::Space));
    assert_eq!(ms(&rig.sink.samples()[0]), 70);
}

#[test]
fn ac19_a_failed_pack_in_a_patch_with_other_fields_applies_none_of_them() {
    let rig = rig_with(Some(&file("full")), &PACKS, |packs| {
        let dir = packs.join("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("pack.json"), json_default(&["down/missing.wav"])).unwrap();
    });
    let before = rig.bytes();
    assert!(
        rig.core
            .set_settings(&json!({"enabled": false, "volume": 0.1, "pack_id": "broken"}))
            .is_err()
    );
    assert_eq!(rig.bytes(), before);
    assert!(rig.live.enabled());
    assert!(approx(rig.live.gain(), 0.64));
}

// AC20: start-up values are in force

#[test]
fn ac20_enabled_false_and_the_volume_from_the_file_are_in_force_from_the_start() {
    let text = file("full")
        .replace("\"enabled\": true", "\"enabled\": false")
        .replace("0.8", "0.5");
    let rig = rig(Some(&text), &PACKS);
    assert!(!rig.live.enabled());
    assert!(approx(rig.live.gain(), 0.25));
    rig.player.handle(down(KeyCode::KeyA));
    assert_eq!(rig.sink.count(), 0);
}

#[test]
fn ac20_the_default_file_plays_at_the_default_volume() {
    let rig = rig(None, &[("brown", "full")]);
    assert!(rig.live.enabled());
    assert!(approx(rig.live.gain(), 0.64));
    rig.player.handle(down(KeyCode::KeyA));
    assert_eq!(rig.sink.count(), 1);
    assert!(approx(rig.sink.samples()[0].gain(), 0.64));
}

#[test]
fn ac20_set_settings_drives_the_live_values() {
    let rig = rig(Some(&file("full")), &PACKS);
    rig.core
        .set_settings(&json!({"enabled": false, "volume": 0.5}))
        .unwrap();
    assert!(!rig.live.enabled());
    assert!(approx(rig.live.gain(), 0.25));
}

// AC21: start-up problems and the untouched file

#[test]
fn ac21_the_report_says_nothing_for_a_good_or_a_new_file() {
    let dir = TempDir::new("report");
    let (_, err) = fake_err();
    let path = dir.path().join("settings.json");
    let created = settings::load_or_create_report(&path, &err);
    assert!(created.problem.is_none());
    assert_eq!(created.settings, Settings::default());
    assert!(path.exists());
    let again = settings::load_or_create_report(&path, &err);
    assert!(again.problem.is_none());
}

#[test]
fn ac21_the_report_names_an_unreadable_file_and_replaced_fields() {
    let dir = TempDir::new("report");
    let (buffer, err) = fake_err();
    let path = dir.path().join("settings.json");

    fs::write(&path, "{ broken").unwrap();
    let loaded = settings::load_or_create_report(&path, &err);
    assert!(matches!(loaded.problem, Some(LoadProblem::Unreadable)));
    assert_eq!(loaded.settings, Settings::default());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ broken");
    assert!(
        written(&buffer).contains("settings:"),
        "the stderr line stays"
    );

    fs::write(&path, file("full").replace("0.8", "7")).unwrap();
    let loaded = settings::load_or_create_report(&path, &err);
    match loaded.problem {
        Some(LoadProblem::Replaced(fields)) => assert_eq!(fields, ["volume"]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ac21_load_or_create_still_returns_just_the_settings() {
    let dir = TempDir::new("report");
    let (_, err) = fake_err();
    let path = dir.path().join("settings.json");
    fs::write(&path, file("full")).unwrap();
    let s: Settings = settings::load_or_create(&path, &err);
    assert_eq!(s.pack_id, "full");
}

#[test]
fn ac21_choose_pack_says_what_is_playing_and_whether_it_fell_back() {
    let packs = packs_folder("choose", &[("full", "full"), ("brown", "default-only")]);
    let (_, err) = fake_err();
    let ok = choose_pack(Some(packs.path()), "full", &err);
    assert_eq!(ok.playing.as_deref(), Some("full"));
    assert!(!ok.fell_back);

    let unknown = choose_pack(Some(packs.path()), "nope", &err);
    assert_eq!(unknown.playing.as_deref(), Some("brown"));
    assert!(unknown.fell_back);

    let empty = packs_folder("choose-empty", &[]);
    let click = choose_pack(Some(empty.path()), "nope", &err);
    assert_eq!(click.playing, None);
    assert!(click.fell_back);

    let none = choose_pack(None, "full", &err);
    assert_eq!(none.playing, None);
    assert!(none.fell_back);
}

#[test]
fn ac21_an_unreadable_file_is_untouched_until_the_first_change_then_complete() {
    let rig = rig(Some("{ this is not json"), &[("brown", "full")]);
    let v = rig.core.get_settings();
    assert_eq!(v["startup"]["settings_unreadable"], true);
    assert_eq!(v["startup"]["replaced_fields"], json!([]));
    // Reading state and listing packs do not touch the file.
    rig.core.list_packs();
    rig.core.get_settings();
    assert_eq!(rig.bytes().unwrap(), b"{ this is not json");

    let result = rig.core.set_settings(&json!({"volume": 0.3})).unwrap();
    assert_eq!(result["saved"], true);
    // The values in use (defaults) plus the change.
    assert_eq!(
        rig.settings(),
        Settings {
            volume: 0.3,
            ..Settings::default()
        }
    );
}

#[test]
fn ac21_replaced_fields_are_reported_and_the_file_is_untouched_until_a_change() {
    let text = file("full")
        .replace("\"volume\": 0.8", "\"volume\": 5")
        .replace("\"key_up_sounds\": false", "\"key_up_sounds\": \"yes\"");
    let rig = rig(Some(&text), &PACKS);
    let v = rig.core.get_settings();
    assert_eq!(v["startup"]["settings_unreadable"], false);
    assert_eq!(
        v["startup"]["replaced_fields"],
        json!(["volume", "key_up_sounds"])
    );
    assert_eq!(v["settings"]["volume"], 0.8);
    assert_eq!(rig.bytes().unwrap(), text.as_bytes());

    rig.core.set_settings(&json!({"enabled": false})).unwrap();
    let s = rig.settings();
    assert!(!s.enabled);
    assert_eq!(s.volume, 0.8);
    assert!(s.key_up_sounds);
    assert_eq!(s.pack_id, "full");
    assert_eq!(s.pitch_variation, 0.2);
}

// AC22: a save that fails

#[test]
fn ac22_a_failed_save_still_applies_the_change_and_says_why() {
    let dir = TempDir::new("blocked");
    // The settings file's parent is a regular file, so it cannot be made.
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let path = blocker.join("settings.json");

    let packs = packs_folder("packs", &PACKS);
    let (_, err) = fake_err();
    let loaded = settings::Loaded {
        settings: Settings {
            pack_id: "full".into(),
            ..Settings::default()
        },
        problem: None,
    };
    let chosen = choose_pack(Some(packs.path()), "full", &err);
    let live = Arc::new(Live::new(chosen.sounds));
    let core = Core::new(CoreParts {
        settings_path: Some(path),
        library: Library::bundled_only(packs.path().to_path_buf()),
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::clone(&live),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    });

    let result = core
        .set_settings(&json!({"volume": 0.5}))
        .expect("not an error");
    assert_eq!(result["saved"], false);
    assert!(!result["save_error"].as_str().unwrap().is_empty());
    assert_eq!(result["settings"]["volume"], 0.5);
    assert!(approx(live.gain(), 0.25));
    assert_eq!(core.get_settings()["settings"]["volume"], 0.5);

    // The next change in the same state fails the same way, and the one
    // after the cause is gone succeeds.
    assert_eq!(
        core.set_settings(&json!({"enabled": false})).unwrap()["saved"],
        false
    );
    fs::remove_file(&blocker).unwrap();
    let ok = core.set_settings(&json!({"enabled": true})).unwrap();
    assert_eq!(ok["saved"], true);
    assert!(ok["save_error"].is_null());
}

#[test]
fn ac22_with_no_settings_path_the_change_applies_and_is_not_saved() {
    let packs = packs_folder("packs", &PACKS);
    let (_, err) = fake_err();
    let chosen = choose_pack(Some(packs.path()), "full", &err);
    let live = Arc::new(Live::new(chosen.sounds));
    let core = Core::new(CoreParts {
        settings_path: None,
        library: Library::bundled_only(packs.path().to_path_buf()),
        loaded: settings::Loaded {
            settings: Settings {
                pack_id: "full".into(),
                ..Settings::default()
            },
            problem: None,
        },
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::clone(&live),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    });
    let result = core.set_settings(&json!({"enabled": false})).unwrap();
    assert_eq!(result["saved"], false);
    assert!(!live.enabled());
}

// Feature 08, AC9: the permission state in get_settings and permission_status

fn core_with_permission(permission: Arc<Permission>) -> Core {
    Core::new(CoreParts {
        settings_path: None,
        library: Library::none(),
        loaded: settings::Loaded {
            settings: Settings::default(),
            problem: None,
        },
        playing: None,
        fell_back: true,
        live: Arc::new(Live::new(SoundSet::single(builtin_click()))),
        permission,
        audio: Box::new(|| OutputStatus::Playing),
    })
}

#[test]
fn ac9_get_settings_and_permission_status_carry_the_same_value_for_each_state() {
    for (state, name) in [
        (PermissionState::NotNeeded, "not_needed"),
        (PermissionState::Needed, "needed"),
        (PermissionState::Granted, "granted"),
        (PermissionState::RestartNeeded, "restart_needed"),
    ] {
        let core = core_with_permission(Arc::new(Permission::new(state)));
        assert_eq!(core.get_settings()["permission"], name);
        assert_eq!(core.permission_status(), name);
    }
}

#[test]
fn ac9_the_value_follows_the_state_as_it_changes() {
    let permission = Arc::new(Permission::new(PermissionState::Needed));
    let core = core_with_permission(Arc::clone(&permission));
    assert_eq!(core.permission_status(), "needed");
    permission.set(PermissionState::Granted);
    assert_eq!(core.permission_status(), "granted");
    assert_eq!(core.get_settings()["permission"], "granted");
}

#[test]
fn ac9_neither_waits_on_a_permission_check_that_is_blocked() {
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    use mechanical_key_sounds_lib::keys::KeyListenError;
    use mechanical_key_sounds_lib::permission::{Keys, Os, Watcher};

    struct Gate {
        entered: Mutex<bool>,
        entered_cv: Condvar,
        open: Mutex<bool>,
        open_cv: Condvar,
    }
    struct BlockedOs(Arc<Gate>);
    impl Os for BlockedOs {
        fn required(&self) -> bool {
            true
        }
        fn granted(&self) -> bool {
            *self.0.entered.lock().unwrap() = true;
            self.0.entered_cv.notify_all();
            let mut open = self.0.open.lock().unwrap();
            while !*open {
                open = self.0.open_cv.wait(open).unwrap();
            }
            false
        }
        fn request(&self) {}
        fn open_pane(&self) -> Result<(), String> {
            Ok(())
        }
    }
    struct NoKeys;
    impl Keys for NoKeys {
        fn start(&mut self) -> Result<(), KeyListenError> {
            Ok(())
        }
        fn stop(&mut self) {}
        fn running(&self) -> bool {
            false
        }
    }

    let gate = Arc::new(Gate {
        entered: Mutex::new(false),
        entered_cv: Condvar::new(),
        open: Mutex::new(false),
        open_cv: Condvar::new(),
    });
    let permission = Arc::new(Permission::new(PermissionState::Needed));
    let core = Arc::new(core_with_permission(Arc::clone(&permission)));
    let (_, err) = fake_err();
    let mut watcher = Watcher::new(
        Arc::new(BlockedOs(Arc::clone(&gate))),
        NoKeys,
        Arc::clone(&permission),
        err,
    );
    let handle = std::thread::spawn(move || {
        watcher.step();
    });
    // Wait until the check is inside the OS call.
    let mut entered = gate.entered.lock().unwrap();
    while !*entered {
        entered = gate.entered_cv.wait(entered).unwrap();
    }
    drop(entered);

    let (tx, rx) = std::sync::mpsc::channel();
    let c = Arc::clone(&core);
    std::thread::spawn(move || {
        let a = c.get_settings()["permission"].clone();
        let b = c.permission_status();
        tx.send((a, b)).unwrap();
    });
    let answered = rx.recv_timeout(Duration::from_secs(5));
    *gate.open.lock().unwrap() = true;
    gate.open_cv.notify_all();
    handle.join().unwrap();
    let (a, b) = answered.expect("answers while the check is blocked");
    assert_eq!(a, "needed");
    assert_eq!(b, "needed");
}

// Feature 09: pitch variation and key-up sounds in the patch, at start-up and
// in the pack list.

use std::sync::Mutex;

fn listen(rig: &Rig) -> Arc<Mutex<Vec<Value>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&heard);
    rig.core.subscribe(Box::new(move |v: &Value| {
        sink.lock().unwrap().push(v.clone())
    }));
    heard
}

#[test]
fn feature_09_ac16_a_patch_with_both_new_fields_is_in_force_saved_and_announced() {
    let rig = rig(Some(&file("full")), &PACKS);
    let heard = listen(&rig);
    let before = rig.core.get_settings()["revision"].as_u64().unwrap();
    let result = rig
        .core
        .set_settings(&json!({"pitch_variation": 0.1, "key_up_sounds": true}))
        .unwrap();
    assert_eq!(result["settings"]["pitch_variation"], 0.1);
    assert_eq!(result["settings"]["key_up_sounds"], true);
    assert_eq!(result["revision"].as_u64().unwrap(), before + 1);
    assert_eq!(result["saved"], true);
    assert!(approx(rig.live.pitch_variation(), 0.1));
    assert!(rig.live.key_up());
    assert_eq!(
        rig.settings(),
        Settings {
            enabled: true,
            pack_id: "full".into(),
            volume: 0.8,
            pitch_variation: 0.1,
            key_up_sounds: true,
            launch_at_login: true,
        },
        "every other field is unchanged"
    );
    let heard = heard.lock().unwrap();
    // The subscriber is told once on subscribing, then once for the change.
    assert_eq!(heard.last().unwrap()["settings"]["pitch_variation"], 0.1);
    assert_eq!(heard.last().unwrap()["revision"], result["revision"]);
}

#[test]
fn feature_09_ac16_each_field_alone_changes_only_itself() {
    let rig = rig(Some(&file("full")), &PACKS);
    rig.core
        .set_settings(&json!({"key_up_sounds": true}))
        .unwrap();
    let s = rig.settings();
    assert!(s.key_up_sounds);
    assert_eq!(s.pitch_variation, 0.2);
    assert!(rig.live.key_up());
    assert!(approx(rig.live.pitch_variation(), 0.2));

    rig.core
        .set_settings(&json!({"pitch_variation": 0.05}))
        .unwrap();
    let s = rig.settings();
    assert_eq!(s.pitch_variation, 0.05);
    assert!(s.key_up_sounds);
    assert_eq!(s.volume, 0.8);
    assert!(approx(rig.live.pitch_variation(), 0.05));
}

#[test]
fn feature_09_ac16_zero_and_the_upper_bound_are_accepted() {
    let rig = rig(Some(&file("full")), &PACKS);
    for p in [json!(0), json!(0.0), json!(0.3)] {
        rig.core
            .set_settings(&json!({ "pitch_variation": p }))
            .unwrap_or_else(|e| panic!("{p} should be accepted: {e}"));
    }
    assert_eq!(rig.settings().pitch_variation, 0.3);
    assert!(approx(rig.live.pitch_variation(), 0.3));
}

fn assert_rejected_09(rig: &Rig, patch: Value) {
    let before = rig.bytes();
    let pitch = rig.live.pitch_variation();
    let key_up = rig.live.key_up();
    let revision = rig.core.get_settings()["revision"].clone();
    let result = rig.core.set_settings(&patch);
    assert!(
        result.is_err(),
        "{patch} should be rejected, got {result:?}"
    );
    assert_eq!(rig.bytes(), before, "{patch}: nothing is written");
    assert_eq!(
        rig.live.pitch_variation(),
        pitch,
        "{patch}: pitch unchanged"
    );
    assert_eq!(rig.live.key_up(), key_up, "{patch}: key-up unchanged");
    assert_eq!(rig.core.get_settings()["revision"], revision);
    let v = rig.core.get_settings();
    assert_eq!(v["settings"]["pitch_variation"], 0.2);
    assert_eq!(v["settings"]["key_up_sounds"], false);
}

#[test]
fn feature_09_ac16_a_pitch_variation_outside_range_or_not_a_number_is_rejected() {
    let rig = rig(Some(&file("full")), &PACKS);
    for p in [
        json!(0.3000001),
        json!(-0.01),
        json!(1),
        json!(1e300),
        json!("0.1"),
        json!("NaN"),
        json!(null),
        json!(true),
        json!([0.1]),
    ] {
        assert_rejected_09(&rig, json!({ "pitch_variation": p }));
    }
}

#[test]
fn feature_09_ac16_key_up_sounds_must_be_a_bool() {
    let rig = rig(Some(&file("full")), &PACKS);
    for v in [json!("true"), json!(1), json!(0), json!(null)] {
        assert_rejected_09(&rig, json!({ "key_up_sounds": v }));
    }
}

#[test]
fn feature_09_ac16_one_bad_new_field_rejects_the_whole_patch() {
    let rig = rig(Some(&file("full")), &PACKS);
    assert_rejected_09(
        &rig,
        json!({"pitch_variation": 0.1, "key_up_sounds": "yes"}),
    );
    assert_rejected_09(&rig, json!({"key_up_sounds": true, "pitch_variation": 0.5}));
    assert_rejected_09(
        &rig,
        json!({"pitch_variation": 0.1, "launch_at_login": false}),
    );
}

#[test]
fn feature_09_ac16_launch_at_login_is_still_refused_by_name() {
    let rig = rig(Some(&file("full")), &PACKS);
    let err = rig
        .core
        .set_settings(&json!({"launch_at_login": false}))
        .unwrap_err();
    assert!(err.contains("launch_at_login"), "{err:?}");
}

#[test]
fn feature_09_ac17_the_file_values_are_in_force_from_the_first_key_event() {
    // `file()` holds pitch_variation 0.2 and key_up_sounds false.
    let rig = rig(Some(&file("full")), &PACKS);
    assert!(approx(rig.live.pitch_variation(), 0.2));
    assert!(!rig.live.key_up());
    rig.player.handle(down(KeyCode::KeyA));
    rig.player.handle(up(KeyCode::KeyA));
    let played = rig.sink.samples();
    assert_eq!(played.len(), 1, "the key-up is silent");
    let speed = played[0].speed();
    assert!((0.8..=1.2).contains(&speed), "speed {speed}");
}

#[test]
fn feature_09_ac17_a_file_with_key_up_on_and_no_pitch_plays_both_directions_unchanged() {
    let text = file("full")
        .replace("\"pitch_variation\": 0.2", "\"pitch_variation\": 0.0")
        .replace("\"key_up_sounds\": false", "\"key_up_sounds\": true");
    let rig = rig(Some(&text), &PACKS);
    assert_eq!(rig.live.pitch_variation(), 0.0);
    assert!(rig.live.key_up());
    rig.player.handle(down(KeyCode::KeyA));
    rig.player.handle(up(KeyCode::KeyA));
    let played = rig.sink.samples();
    assert_eq!(played.len(), 2);
    assert!(played.iter().all(|s| s.speed() == 1.0));
}

#[test]
fn feature_09_ac16_set_settings_drives_the_live_values_for_the_next_key() {
    let text = file("full").replace("\"key_up_sounds\": false", "\"key_up_sounds\": true");
    let rig = rig(Some(&text), &PACKS);
    rig.core
        .set_settings(&json!({"key_up_sounds": false, "pitch_variation": 0.0}))
        .unwrap();
    rig.player.handle(down(KeyCode::KeyA));
    rig.player.handle(up(KeyCode::KeyA));
    assert_eq!(rig.sink.count(), 1);
    assert_eq!(rig.sink.samples()[0].speed(), 1.0);
}

#[test]
fn feature_09_ac18_every_listed_pack_says_whether_it_has_key_up_sounds() {
    let rig = rig(
        Some(&file("full")),
        &[
            ("full", "full"),
            ("plain", "default-only"),
            ("noup", "no-up"),
            ("spaceup", "up-space-only"),
        ],
    );
    let list = rig.core.list_packs();
    let flag = |id: &str| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id} is listed"))["has_key_up"]
            .clone()
    };
    assert_eq!(flag("full"), true);
    assert_eq!(flag("spaceup"), true, "one group is enough");
    assert_eq!(flag("plain"), false, "no up section");
    assert_eq!(flag("noup"), false);
}

#[test]
fn feature_09_ac18_empty_up_groups_do_not_count_and_no_audio_is_opened() {
    let rig = rig_with(Some(&file("full")), &[], |packs| {
        let dir = packs.join("emptyup");
        fs::create_dir_all(&dir).unwrap();
        // The listed files do not exist: listing must not open audio.
        fs::write(
            dir.join("pack.json"),
            r#"{"name":"Empty up","license":"CC0-1.0",
                "keys":{"default":["down/missing.wav"]},
                "up":{"default":[],"space":[]}}"#,
        )
        .unwrap();
        let dir = packs.join("hasup");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("pack.json"),
            r#"{"name":"Has up","license":"CC0-1.0",
                "keys":{"default":["down/missing.wav"]},
                "up":{"default":[],"enter":["up/missing.wav"]}}"#,
        )
        .unwrap();
    });
    let list = rig.core.list_packs();
    let rows = list.as_array().unwrap();
    let flag = |id: &str| rows.iter().find(|r| r["id"] == id).unwrap()["has_key_up"].clone();
    assert_eq!(flag("emptyup"), false);
    assert_eq!(flag("hasup"), true);
}
