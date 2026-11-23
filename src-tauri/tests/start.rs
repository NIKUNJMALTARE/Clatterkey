//! Tests for the start-up gate and the building of the `Core` (feature 06
//! plan, Risks: commands must never wait for ever). Interfaces:
//! `CoreGate::{new, publish, wait}`, `start_core`, `plain_core`,
//! `publish_core`.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use mechanical_key_sounds_lib::audio::Player;
use mechanical_key_sounds_lib::audio::{Live, OutputStatus};
use mechanical_key_sounds_lib::commands::{
    AudioStatus, Core, CoreGate, plain_core, publish_core, start_core,
};
use mechanical_key_sounds_lib::keys::KeyCode;
use serde_json::json;

mod common;
use common::*;

fn playing() -> AudioStatus {
    Box::new(|| OutputStatus::Playing)
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn file(pack: &str) -> String {
    format!(
        "{{\n  \"enabled\": true,\n  \"pack_id\": {pack:?},\n  \"volume\": 0.8,\n  \"pitch_variation\": 0.0,\n  \"key_up_sounds\": true,\n  \"launch_at_login\": false\n}}\n"
    )
}

// The gate

#[test]
fn wait_blocks_until_publish_and_wakes_every_waiter() {
    let gate = Arc::new(CoreGate::new());
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    for _ in 0..4 {
        let (gate, tx) = (Arc::clone(&gate), tx.clone());
        handles.push(std::thread::spawn(move || {
            let core = gate.wait();
            tx.send(core.audio_status()).unwrap();
        }));
    }
    assert!(
        rx.recv_timeout(Duration::from_millis(150)).is_err(),
        "nobody gets a Core before it is published"
    );
    let (core, _live) = plain_core(playing());
    gate.publish(Arc::new(core));
    for _ in 0..4 {
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "playing");
    }
    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn wait_after_publish_returns_at_once_and_the_first_published_stays() {
    let gate = CoreGate::new();
    let (first, _) = plain_core(Box::new(|| OutputStatus::NoDevice));
    let (second, _) = plain_core(playing());
    gate.publish(Arc::new(first));
    gate.publish(Arc::new(second));
    assert_eq!(gate.wait().audio_status(), "no_device");
    assert_eq!(gate.wait().audio_status(), "no_device");
}

// plain_core

#[test]
fn plain_core_is_the_defaults_on_the_built_in_click_with_nowhere_to_save() {
    let (core, live) = plain_core(playing());
    let v = core.get_settings();
    assert_eq!(v["settings"]["volume"], 0.8);
    assert_eq!(v["settings"]["enabled"], true);
    assert!(v["playing_pack"].is_null());
    assert_eq!(core.list_packs(), json!([]));
    assert!(live.enabled());
    assert!(approx(live.gain(), 0.64));
    let r = core.set_settings(&json!({"enabled": false})).unwrap();
    assert_eq!(r["saved"], false);
    assert!(!live.enabled());
}

// start_core

#[test]
fn start_core_reads_the_file_and_plays_the_chosen_pack() {
    let dir = TempDir::new("start");
    let packs = packs_folder(
        "start-packs",
        &[("full", "full"), ("brown", "default-only")],
    );
    let path = dir.path().join("settings.json");
    fs::write(&path, file("full")).unwrap();
    let (_, err) = fake_err();
    let (core, live) = start_core(
        Some(packs.path().to_path_buf()),
        Some(path),
        playing(),
        &err,
    );
    assert_eq!(core.get_settings()["playing_pack"], "full");
    let sink = Arc::new(RecordingSink::default());
    Player::with_live(sink.clone(), live).handle(down(KeyCode::Space));
    assert_eq!(ms(&sink.samples()[0]), 70);
}

#[test]
fn start_core_with_no_settings_path_uses_defaults_and_says_so() {
    let packs = packs_folder("start-packs", &[("brown", "full")]);
    let (buffer, err) = fake_err();
    let (core, live) = start_core(Some(packs.path().to_path_buf()), None, playing(), &err);
    assert_eq!(core.get_settings()["playing_pack"], "brown");
    assert!(live.enabled());
    assert!(written(&buffer).contains("settings:"));
    let r = core.set_settings(&json!({"volume": 0.5})).unwrap();
    assert_eq!(r["saved"], false);
}

#[test]
fn start_core_falls_back_to_the_default_pack() {
    let dir = TempDir::new("start");
    let packs = packs_folder("start-packs", &[("brown", "full")]);
    let path = dir.path().join("settings.json");
    fs::write(&path, file("nope")).unwrap();
    let (_, err) = fake_err();
    let (core, _live) = start_core(
        Some(packs.path().to_path_buf()),
        Some(path.clone()),
        playing(),
        &err,
    );
    let v = core.get_settings();
    assert_eq!(v["playing_pack"], "brown");
    assert_eq!(v["startup"]["unusable_pack"], "nope");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        file("nope"),
        "file untouched"
    );
}

#[test]
fn start_core_falls_back_to_the_built_in_click_with_no_packs() {
    let dir = TempDir::new("start");
    let packs = packs_folder("start-packs", &[]);
    let path = dir.path().join("settings.json");
    fs::write(&path, file("nope")).unwrap();
    let (_, err) = fake_err();
    let (core, live) = start_core(
        Some(packs.path().to_path_buf()),
        Some(path),
        playing(),
        &err,
    );
    let v = core.get_settings();
    assert!(v["playing_pack"].is_null());
    assert_eq!(v["startup"]["unusable_pack"], "nope");
    let sink = Arc::new(RecordingSink::default());
    Player::with_live(sink.clone(), live).handle(down(KeyCode::KeyA));
    assert_eq!(sink.count(), 1);
}

#[test]
fn start_core_falls_back_to_the_built_in_click_with_no_packs_folder() {
    let dir = TempDir::new("start");
    let (_, err) = fake_err();
    let (core, _live) = start_core(
        None,
        Some(dir.path().join("settings.json")),
        playing(),
        &err,
    );
    assert!(core.get_settings()["playing_pack"].is_null());
    assert_eq!(core.list_packs(), json!([]));
}

// publish_core

fn published(gate: &CoreGate) -> Arc<Core> {
    // The gate must already be open: wait on another thread with a limit.
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|s| {
        s.spawn(|| tx.send(gate.wait()).unwrap());
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the gate is open")
    })
}

#[test]
fn publish_core_publishes_what_build_made_and_returns_its_live() {
    let dir = TempDir::new("start");
    let packs = packs_folder(
        "start-packs",
        &[("full", "full"), ("brown", "default-only")],
    );
    let path = dir.path().join("settings.json");
    fs::write(&path, file("full")).unwrap();
    let (buffer, err) = fake_err();
    let gate = CoreGate::new();
    let live = publish_core(&gate, playing, &err, |audio| {
        start_core(
            Some(packs.path().to_path_buf()),
            Some(path.clone()),
            audio,
            &err,
        )
    });
    let core = published(&gate);
    assert_eq!(core.get_settings()["playing_pack"], "full");
    assert_eq!(written(&buffer), "");
    core.set_settings(&json!({"enabled": false, "volume": 0.5}))
        .unwrap();
    assert!(!live.enabled());
    assert!(approx(live.gain(), 0.25));
}

#[test]
fn publish_core_publishes_a_core_when_the_pack_choice_falls_back() {
    let dir = TempDir::new("start");
    let packs = packs_folder("start-packs", &[("brown", "full")]);
    let path = dir.path().join("settings.json");
    fs::write(&path, file("nope")).unwrap();
    let (_, err) = fake_err();
    let gate = CoreGate::new();
    let live = publish_core(&gate, playing, &err, |audio| {
        start_core(
            Some(packs.path().to_path_buf()),
            Some(path.clone()),
            audio,
            &err,
        )
    });
    let core = published(&gate);
    assert_eq!(core.get_settings()["playing_pack"], "brown");
    core.set_settings(&json!({"volume": 0.5})).unwrap();
    assert!(approx(live.gain(), 0.25));
}

#[test]
fn publish_core_publishes_a_core_with_no_packs_or_no_packs_folder() {
    for packs in [Some(packs_folder("start-packs", &[])), None] {
        let dir = TempDir::new("start");
        let path = dir.path().join("settings.json");
        let (_, err) = fake_err();
        let gate = CoreGate::new();
        let live = publish_core(&gate, playing, &err, |audio| {
            start_core(
                packs.as_ref().map(|p| p.path().to_path_buf()),
                Some(path.clone()),
                audio,
                &err,
            )
        });
        let core = published(&gate);
        assert!(core.get_settings()["playing_pack"].is_null());
        core.set_settings(&json!({"enabled": false})).unwrap();
        assert!(!live.enabled());
    }
}

#[test]
fn publish_core_publishes_a_core_when_there_is_no_settings_path() {
    let packs = packs_folder("start-packs", &[("brown", "full")]);
    let (_, err) = fake_err();
    let gate = CoreGate::new();
    let live = publish_core(&gate, playing, &err, |audio| {
        start_core(Some(packs.path().to_path_buf()), None, audio, &err)
    });
    let core = published(&gate);
    let r = core.set_settings(&json!({"enabled": false})).unwrap();
    assert_eq!(r["saved"], false);
    assert!(!live.enabled());
}

#[test]
fn publish_core_when_build_panics_writes_one_line_and_publishes_a_plain_core() {
    let (buffer, err) = fake_err();
    let gate = CoreGate::new();
    let called = AtomicBool::new(false);
    let live = publish_core(&gate, playing, &err, |_audio| -> (Core, Arc<Live>) {
        called.store(true, Ordering::SeqCst);
        panic!("start-up exploded");
    });
    assert!(called.load(Ordering::SeqCst));
    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "{text:?}");
    let core = published(&gate);
    assert!(core.get_settings()["playing_pack"].is_null());
    assert_eq!(core.list_packs(), json!([]));
    // The returned Live is the one the published Core changes.
    core.set_settings(&json!({"enabled": false, "volume": 0.5}))
        .unwrap();
    assert!(!live.enabled());
    assert!(approx(live.gain(), 0.25));
}

#[test]
fn publish_core_uses_the_audio_reader_it_is_given() {
    let (_, err) = fake_err();
    let gate = CoreGate::new();
    publish_core(
        &gate,
        || Box::new(|| OutputStatus::NoDevice),
        &err,
        plain_core,
    );
    assert_eq!(published(&gate).audio_status(), "no_device");
}
