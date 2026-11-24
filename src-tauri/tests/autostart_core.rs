//! Tests for launch at login on the `Core` (feature 11: AC9 to AC13, AC16,
//! AC19 core half, AC20) over a recording fake registration, temporary
//! settings files, fixture packs and a subscriber. No test touches the real
//! login registration (AC21).
//!
//! Interfaces assumed from the plan:
//!
//! ```ignore
//! CoreParts { .., login_item: Arc<dyn LoginItem> }
//! start_core(Library, settings_path, audio, permission, login_item, err)
//! set_settings accepts "launch_at_login": bool
//! error: "settings change: launch at login could not be turned on|off: <e>"
//! ```

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clickclak_lib::audio::OutputStatus;
use clickclak_lib::autostart::{Inert, LoginItem};
use clickclak_lib::commands::{AudioStatus, Core, CoreParts, start_core};
use clickclak_lib::keys::KeyCode;
use clickclak_lib::packs::Library;
use clickclak_lib::permission::{Permission, PermissionState};
use clickclak_lib::settings::{self, Settings};
use clickclak_lib::startup::choose_pack;
use serde_json::{Value, json};

mod common;
use common::*;
mod support;
use support::*;

const PACKS: [(&str, &str); 2] = [("full", "full"), ("plain", "default-only")];

/// `file()` has `launch_at_login: true`; this one has it off.
fn file_off(pack: &str) -> String {
    file(pack).replace("\"launch_at_login\": true", "\"launch_at_login\": false")
}

fn rig_over(text: &str, item: &Arc<FakeItem>) -> Rig {
    rig_item(Some(text), &PACKS, |_| (), item.clone())
}

fn listen(rig: &Rig) -> Arc<Mutex<Vec<Value>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&heard);
    rig.core.subscribe(Box::new(move |v: &Value| {
        sink.lock().unwrap().push(v.clone())
    }));
    heard
}

fn playing() -> AudioStatus {
    Box::new(|| OutputStatus::Playing)
}

fn perm() -> Arc<Permission> {
    Arc::new(Permission::new(PermissionState::NotNeeded))
}

fn revision(rig: &Rig) -> Value {
    rig.core.get_settings()["revision"].clone()
}

// AC9: a patch turns the registration on or off

#[test]
fn ac9_turning_it_on_registers_once_saves_and_tells_subscribers() {
    let item = FakeItem::new(false);
    let rig = rig_over(&file_off("full"), &item);
    let heard = listen(&rig);
    let before = revision(&rig).as_u64().unwrap();
    let result = rig
        .core
        .set_settings(&json!({"launch_at_login": true}))
        .unwrap();
    assert_eq!(item.calls(), ["enable"]);
    assert_eq!(result["settings"]["launch_at_login"], true);
    assert_eq!(result["saved"], true);
    assert_eq!(result["revision"], before + 1);
    assert_eq!(rig.core.get_settings()["settings"]["launch_at_login"], true);
    assert_eq!(
        rig.settings(),
        Settings {
            enabled: true,
            pack_id: "full".into(),
            volume: 0.8,
            pitch_variation: 0.2,
            key_up_sounds: false,
            launch_at_login: true,
        },
        "every other field is unchanged"
    );
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 2, "once on subscribing, once for the change");
    assert_eq!(heard[1]["settings"]["launch_at_login"], true);
    assert_eq!(heard[1]["revision"], before + 1);
}

#[test]
fn ac9_turning_it_off_removes_it_once_saves_and_tells_subscribers() {
    let item = FakeItem::new(true);
    let rig = rig_over(&file("full"), &item);
    let heard = listen(&rig);
    let before = revision(&rig).as_u64().unwrap();
    let result = rig
        .core
        .set_settings(&json!({"launch_at_login": false}))
        .unwrap();
    assert_eq!(item.calls(), ["disable"]);
    assert_eq!(result["settings"]["launch_at_login"], false);
    assert_eq!(result["saved"], true);
    assert_eq!(result["revision"], before + 1);
    assert_eq!(
        rig.settings(),
        Settings {
            enabled: true,
            pack_id: "full".into(),
            volume: 0.8,
            pitch_variation: 0.2,
            key_up_sounds: false,
            launch_at_login: false,
        }
    );
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[1]["settings"]["launch_at_login"], false);
}

// AC10: the system refuses

#[test]
fn ac10_a_failed_registration_rejects_the_patch_and_changes_nothing() {
    let item = FakeItem::new(false);
    item.fail_enable("permission denied (os error 13)");
    let rig = rig_over(&file_off("full"), &item);
    let heard = listen(&rig);
    let bytes = rig.bytes();
    let rev = revision(&rig);
    let err = rig
        .core
        .set_settings(&json!({"launch_at_login": true}))
        .unwrap_err();
    assert!(err.contains("launch at login"), "{err:?}");
    assert!(err.contains("permission denied (os error 13)"), "{err:?}");
    assert_eq!(rig.bytes(), bytes, "the file is unchanged");
    assert_eq!(
        rig.core.get_settings()["settings"]["launch_at_login"],
        false
    );
    assert_eq!(revision(&rig), rev, "the revision does not rise");
    assert_eq!(heard.lock().unwrap().len(), 1, "only the first telling");
}

#[test]
fn ac10_a_failed_removal_rejects_the_patch_and_changes_nothing() {
    let item = FakeItem::new(true);
    item.fail_disable("registry key is locked");
    let rig = rig_over(&file("full"), &item);
    let heard = listen(&rig);
    let bytes = rig.bytes();
    let rev = revision(&rig);
    let err = rig
        .core
        .set_settings(&json!({"launch_at_login": false}))
        .unwrap_err();
    assert!(err.contains("launch at login"), "{err:?}");
    assert!(err.contains("registry key is locked"), "{err:?}");
    assert_eq!(rig.bytes(), bytes);
    assert_eq!(rig.core.get_settings()["settings"]["launch_at_login"], true);
    assert_eq!(revision(&rig), rev);
    assert_eq!(heard.lock().unwrap().len(), 1);
}

#[test]
fn ac10_a_rejected_patch_leaves_the_other_fields_in_the_patch_unapplied() {
    let item = FakeItem::new(false);
    item.fail_enable("no");
    let rig = rig_over(&file_off("full"), &item);
    rig.core
        .set_settings(&json!({"volume": 0.1, "launch_at_login": true}))
        .unwrap_err();
    assert_eq!(rig.core.get_settings()["settings"]["volume"], 0.8);
    assert_eq!(rig.settings().volume, 0.8);
    assert!((rig.live.gain() - 0.64).abs() < 1e-6, "gain is unchanged");
}

// AC11: checks first, equal values, other types

#[test]
fn ac11_an_invalid_companion_field_rejects_the_whole_patch_without_touching_the_registration() {
    let item = FakeItem::new(false);
    let rig = rig_over(&file_off("full"), &item);
    let bytes = rig.bytes();
    for patch in [
        json!({"launch_at_login": true, "volume": 2.0}),
        json!({"launch_at_login": true, "enabled": "yes"}),
        json!({"launch_at_login": true, "bogus": 1}),
    ] {
        rig.core.set_settings(&patch).unwrap_err();
    }
    assert!(item.calls().is_empty(), "{:?}", item.calls());
    assert_eq!(rig.bytes(), bytes);
    assert_eq!(
        rig.core.get_settings()["settings"]["launch_at_login"],
        false
    );
}

#[test]
fn ac11_launch_at_login_must_be_a_bool() {
    let item = FakeItem::new(false);
    let rig = rig_over(&file_off("full"), &item);
    let bytes = rig.bytes();
    for v in [
        json!("true"),
        json!(1),
        json!(0),
        json!(null),
        json!([true]),
    ] {
        let err = rig
            .core
            .set_settings(&json!({ "launch_at_login": v }))
            .unwrap_err();
        assert!(!err.is_empty());
    }
    assert!(item.calls().is_empty());
    assert_eq!(rig.bytes(), bytes);
}

#[test]
fn ac11_a_value_equal_to_the_one_in_force_still_brings_the_registration_into_line() {
    // Setting on, registration gone (changed in the system): on is asked again.
    let item = FakeItem::new(false);
    let rig = rig_over(&file("full"), &item);
    let result = rig
        .core
        .set_settings(&json!({"launch_at_login": true}))
        .expect("a success");
    assert_eq!(item.calls(), ["enable"]);
    assert_eq!(result["settings"]["launch_at_login"], true);

    // Setting off, a registration still there: off removes it.
    let item = FakeItem::new(true);
    let rig = rig_over(&file_off("full"), &item);
    let result = rig
        .core
        .set_settings(&json!({"launch_at_login": false}))
        .expect("a success");
    assert_eq!(item.calls(), ["disable"]);
    assert_eq!(result["settings"]["launch_at_login"], false);
}

#[test]
fn ac11_edge_an_unknown_pack_with_launch_at_login_is_rejected_and_the_fake_never_called() {
    let item = FakeItem::new(false);
    let rig = rig_over(&file_off("full"), &item);
    rig.core
        .set_settings(&json!({"launch_at_login": true, "pack_id": "nope"}))
        .unwrap_err();
    assert!(item.calls().is_empty());
    assert_eq!(
        rig.core.get_settings()["settings"]["launch_at_login"],
        false
    );
}

#[test]
fn ac11_edge_a_pack_that_fails_to_load_with_launch_at_login_is_rejected_and_the_fake_never_called()
{
    let item = FakeItem::new(false);
    let rig = rig_item(
        Some(&file_off("full")),
        &PACKS,
        |packs| {
            let broken = packs.join("broken");
            copy_dir(&fixture_pack("full"), &broken);
            fs::remove_dir_all(broken.join("down")).unwrap();
        },
        item.clone(),
    );
    rig.core
        .set_settings(&json!({"launch_at_login": true, "pack_id": "broken"}))
        .unwrap_err();
    assert!(item.calls().is_empty(), "{:?}", item.calls());
    assert_eq!(
        rig.core.get_settings()["settings"]["launch_at_login"],
        false
    );
    assert_eq!(rig.core.get_settings()["settings"]["pack_id"], "full");
}

#[test]
fn ac11_edge_two_flips_in_a_row_end_with_the_last_one() {
    let item = FakeItem::new(false);
    let rig = rig_over(&file_off("full"), &item);
    rig.core
        .set_settings(&json!({"launch_at_login": true}))
        .unwrap();
    rig.core
        .set_settings(&json!({"launch_at_login": false}))
        .unwrap();
    assert_eq!(item.calls(), ["enable", "disable"]);
    assert!(!rig.settings().launch_at_login);
    assert_eq!(*item.in_force.lock().unwrap(), Ok(false));
}

// AC12: other patches never reach the registration

#[test]
fn ac12_a_patch_without_the_field_neither_reads_nor_changes_the_registration() {
    let item = FakeItem::new(true);
    let rig = rig_over(&file("full"), &item);
    for patch in [
        json!({"volume": 0.5}),
        json!({"enabled": false}),
        json!({"pack_id": "plain"}),
        json!({"pitch_variation": 0.1, "key_up_sounds": true}),
    ] {
        rig.core.set_settings(&patch).unwrap();
    }
    assert!(item.calls().is_empty(), "{:?}", item.calls());
}

#[test]
fn ac12_edge_toggle_enabled_never_calls_the_registration() {
    let item = FakeItem::new(true);
    let rig = rig_over(&file("full"), &item);
    rig.core.toggle_enabled();
    rig.core.toggle_enabled();
    assert!(item.calls().is_empty(), "{:?}", item.calls());
}

// AC13: the save fails after the registration is made

#[test]
fn ac13_a_failed_save_keeps_the_change_and_the_registration() {
    let dir = TempDir::new("blocked");
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let path = blocker.join("settings.json");
    let packs = packs_folder("packs", &PACKS);
    let (_, err) = fake_err();
    let chosen = choose_pack(Some(packs.path()), "full", &err);
    let item = FakeItem::new(false);
    let core = Core::new(CoreParts {
        settings_path: Some(path),
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
        live: Arc::new(clickclak_lib::audio::Live::new(chosen.sounds)),
        permission: perm(),
        audio: playing(),
        login_item: item.clone(),
    });
    let result = core
        .set_settings(&json!({"launch_at_login": true}))
        .expect("not an error");
    assert_eq!(result["saved"], false);
    assert!(!result["save_error"].as_str().unwrap().is_empty());
    assert_eq!(result["settings"]["launch_at_login"], true);
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], true);
    assert_eq!(item.calls(), ["enable"], "the registration is kept");
}

// AC19: development builds, a Core over Inert

#[test]
fn ac19_a_patch_over_inert_succeeds_and_is_saved() {
    let rig = rig_item(Some(&file_off("full")), &PACKS, |_| (), Arc::new(Inert));
    let on = rig
        .core
        .set_settings(&json!({"launch_at_login": true}))
        .unwrap();
    assert_eq!(on["settings"]["launch_at_login"], true);
    assert_eq!(on["saved"], true);
    assert!(rig.settings().launch_at_login);
    let off = rig
        .core
        .set_settings(&json!({"launch_at_login": false}))
        .unwrap();
    assert_eq!(off["settings"]["launch_at_login"], false);
    assert!(!rig.settings().launch_at_login);
}

// AC16 and AC14 at start-up, over start_core

fn start(
    text: Option<&str>,
    item: &Arc<FakeItem>,
) -> (Core, (TempDir, TempDir), std::path::PathBuf, Captured) {
    let dir = TempDir::new("cfg");
    let packs = packs_folder("packs", &PACKS);
    let path = dir.path().join("settings.json");
    if let Some(text) = text {
        fs::write(&path, text).unwrap();
    }
    let (buffer, err) = fake_err();
    let (core, _live) = start_core(
        Library::bundled_only(packs.path().to_path_buf()),
        Some(path.clone()),
        playing(),
        perm(),
        item.clone(),
        &err,
    );
    (core, (dir, packs), path, buffer)
}

#[test]
fn ac16_the_file_says_on_and_the_system_says_none_so_the_first_read_says_off_and_the_file_is_rewritten()
 {
    let item = FakeItem::new(false);
    let (core, _dir, path, buffer) = start(Some(&file("full")), &item);
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], false);
    assert!(!settings::read(&path).unwrap().settings.launch_at_login);
    assert!(!item.calls().contains(&"enable"), "nothing is registered");
    assert!(!written(&buffer).contains("autostart:"), "no line either");
    let v = core.get_settings();
    assert_eq!(v["startup"]["settings_unreadable"], false);
    assert_eq!(v["startup"]["replaced_fields"], json!([]));
}

#[test]
fn ac14_on_with_a_registration_stays_on_and_is_refreshed_once() {
    let item = FakeItem::new(true);
    let (core, _dir, path, _) = start(Some(&file("full")), &item);
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], true);
    assert!(settings::read(&path).unwrap().settings.launch_at_login);
    assert_eq!(item.calls().iter().filter(|c| **c == "enable").count(), 1);
}

#[test]
fn ac14_off_with_a_registration_removes_it_and_leaves_the_file_off() {
    let item = FakeItem::new(true);
    let (core, _dir, path, _) = start(Some(&file_off("full")), &item);
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], false);
    assert!(!settings::read(&path).unwrap().settings.launch_at_login);
    assert_eq!(item.calls().iter().filter(|c| **c == "disable").count(), 1);
    assert_eq!(*item.in_force.lock().unwrap(), Ok(false));
}

#[test]
fn ac14_off_with_none_changes_nothing() {
    let item = FakeItem::new(false);
    let text = file_off("full");
    let (core, _dir, path, _) = start(Some(&text), &item);
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], false);
    assert_eq!(fs::read_to_string(path).unwrap(), text);
    assert!(!item.calls().contains(&"enable"));
    assert!(!item.calls().contains(&"disable"));
}

#[test]
fn ac15_a_system_that_cannot_be_asked_leaves_the_setting_and_start_up_carries_on() {
    let item = FakeItem::new(true);
    item.cannot_be_asked("service is down");
    let text = file("full");
    let (core, _dir, path, buffer) = start(Some(&text), &item);
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], true);
    assert_eq!(core.get_settings()["playing_pack"], "full");
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        text,
        "the file is as it was"
    );
    let lines: Vec<String> = written(&buffer)
        .lines()
        .filter(|l| l.starts_with("autostart:"))
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
}

#[test]
fn ac19_start_up_over_inert_leaves_the_setting_as_it_is() {
    let dir = TempDir::new("cfg");
    let packs = packs_folder("packs", &PACKS);
    let path = dir.path().join("settings.json");
    let text = file("full");
    fs::write(&path, &text).unwrap();
    let (_, err) = fake_err();
    let (core, _live) = start_core(
        Library::bundled_only(packs.path().to_path_buf()),
        Some(path.clone()),
        playing(),
        perm(),
        Arc::new(Inert),
        &err,
    );
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], true);
    assert_eq!(fs::read_to_string(path).unwrap(), text);
}

#[test]
fn ac14_edge_an_unreadable_file_with_a_registration_in_force_removes_it_and_is_not_rewritten() {
    let item = FakeItem::new(true);
    let garbage = "{ this is not json";
    let (core, _dir, path, _) = start(Some(garbage), &item);
    // Defaults have the setting off, so the registration goes.
    assert_eq!(core.get_settings()["settings"]["launch_at_login"], false);
    assert_eq!(item.calls().iter().filter(|c| **c == "disable").count(), 1);
    assert_eq!(core.get_settings()["startup"]["settings_unreadable"], true);
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        garbage,
        "never rewritten"
    );
}

#[test]
fn ac14_edge_with_no_settings_path_the_comparison_runs_and_nothing_is_saved() {
    let item = FakeItem::new(true);
    let packs = packs_folder("packs", &PACKS);
    let (_, err) = fake_err();
    let (core, _live) = start_core(
        Library::bundled_only(packs.path().to_path_buf()),
        None,
        playing(),
        perm(),
        item.clone(),
        &err,
    );
    assert!(
        !core.get_settings()["settings"]["launch_at_login"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(item.calls().iter().filter(|c| **c == "disable").count(), 1);
}

// AC20: the key path takes no lock the registration can hold

struct Blocking {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}

impl LoginItem for Blocking {
    fn enable(&self) -> Result<(), String> {
        self.entered.store(true, Ordering::SeqCst);
        let until = Instant::now() + Duration::from_secs(10);
        while !self.release.load(Ordering::SeqCst) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    fn disable(&self) -> Result<(), String> {
        Ok(())
    }
    fn is_enabled(&self) -> Result<bool, String> {
        Ok(false)
    }
}

#[test]
fn ac20_key_events_are_played_while_the_registration_blocks() {
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let item = Arc::new(Blocking {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let rig = rig_item(Some(&file_off("full")), &PACKS, |_| (), item);
    let core = Arc::clone(&rig.core);
    let patch = std::thread::spawn(move || core.set_settings(&json!({"launch_at_login": true})));
    let until = Instant::now() + Duration::from_secs(5);
    while !entered.load(Ordering::SeqCst) {
        assert!(Instant::now() < until, "the registration was never reached");
        std::thread::sleep(Duration::from_millis(2));
    }

    let started = Instant::now();
    rig.player.handle(down(KeyCode::KeyA));
    rig.player.handle(up(KeyCode::KeyA));
    let took = started.elapsed();
    let played = rig.sink.count();
    release.store(true, Ordering::SeqCst);
    patch.join().unwrap().unwrap();

    assert!(
        took < Duration::from_secs(1),
        "the key path waited {took:?}"
    );
    assert!(played >= 1, "the key was played while the patch blocked");
}
