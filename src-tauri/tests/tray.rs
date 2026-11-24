//! Tests for the tray and background feature's core side (feature 07: AC1 to
//! AC10), over temporary settings files, fixture packs and a recording sink.
//! No real tray, window or Tauri runtime is created.
//!
//! Interfaces assumed from the feature 07 plan:
//!
//! ```ignore
//! // commands
//! Core::toggle_enabled(&self) -> Value
//! Core::subscribe(&self, Listener)   // Listener = Box<dyn Fn(&Value) + Send + Sync>
//! // results and get_settings carry "revision": u64, starting at 0
//! // tray (model layer, re-exported from `tray`)
//! TrayIconKind::{Normal, Off}; TrayView { checked, icon, tooltip }; TrayView::of(bool)
//! TrayAction::{ToggleSound, ShowSettings, Quit}
//! MenuEntry::{Item { id, label, action }, Separator}; MENU; action_for(&str)
//! TraySync::new() -> TraySync; TraySync::update(&self, &Value) -> Option<TrayView>
//! toggle_sound(&CoreGate, &ErrorOut) -> Value
//! CloseDecision::{Hide, Close}; on_close_request(bool)
//! ExitDecision::{Stay, Exit}; on_exit_request(quit_asked, tray_exists)
//! ```

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clickclak_lib::audio::{Live, OutputStatus, Player};
use clickclak_lib::autostart::Inert;
use clickclak_lib::commands::{Core, CoreGate, CoreParts, Listener};
use clickclak_lib::keys::KeyCode;
use clickclak_lib::packs::Library;
use clickclak_lib::permission::{Permission, PermissionState};
use clickclak_lib::settings::{self, Settings};
use clickclak_lib::startup::choose_pack;
use clickclak_lib::tray::{
    CloseDecision, ExitDecision, MENU, MenuEntry, TrayAction, TrayIconKind, TraySync, TrayView,
    action_for, on_close_request, on_exit_request, toggle_sound,
};
use serde_json::{Value, json};

mod common;
use common::*;
mod support;
use support::*;

const PACKS: [(&str, &str); 2] = [("full", "full"), ("plain", "default-only")];

/// A listener that records every result it is given.
fn recorder() -> (Arc<Mutex<Vec<Value>>>, Listener) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    (
        seen,
        Box::new(move |v| sink.lock().unwrap().push(v.clone())),
    )
}

fn gate_with(core: &Arc<Core>) -> Arc<CoreGate> {
    let gate = Arc::new(CoreGate::new());
    gate.publish(Arc::clone(core));
    gate
}

fn enabled_of(v: &Value) -> bool {
    v["settings"]["enabled"].as_bool().unwrap()
}

/// A core whose settings file cannot be written.
fn blocked_rig() -> (
    Arc<Core>,
    Arc<Live>,
    Player,
    Arc<RecordingSink>,
    TempDir,
    TempDir,
) {
    let dir = TempDir::new("blocked");
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
    let core = Arc::new(Core::new(CoreParts {
        settings_path: Some(path),
        library: Library::bundled_only(packs.path().to_path_buf()),
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        login_item: Arc::new(Inert),
        live: Arc::clone(&live),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    }));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    (core, live, player, sink, dir, packs)
}

// AC1: what the tray shows

#[test]
fn ac1_sound_on_shows_a_tick_the_normal_icon_and_the_plain_tooltip() {
    let view = TrayView::of(true);
    assert!(view.checked);
    assert_eq!(view.icon, TrayIconKind::Normal);
    assert_eq!(view.tooltip, "ClickClak");
}

#[test]
fn ac1_sound_off_shows_no_tick_the_dimmed_icon_and_the_off_tooltip() {
    let view = TrayView::of(false);
    assert!(!view.checked);
    assert_eq!(view.icon, TrayIconKind::Off);
    assert_eq!(view.tooltip, "ClickClak (sound off)");
}

// AC2: the menu

#[test]
fn ac2_the_menu_is_sound_settings_separator_quit_in_that_order() {
    assert_eq!(MENU.len(), 4);
    match &MENU[0] {
        MenuEntry::Item { id, label, action } => {
            assert_eq!((*id, *label), ("sound", "Sound"));
            assert!(matches!(action, TrayAction::ToggleSound));
        }
        _ => panic!("first entry must be Sound"),
    }
    match &MENU[1] {
        MenuEntry::Item { id, label, action } => {
            assert_eq!((*id, *label), ("settings", "Settings\u{2026}"));
            assert!(matches!(action, TrayAction::ShowSettings));
        }
        _ => panic!("second entry must be Settings"),
    }
    assert!(matches!(MENU[2], MenuEntry::Separator));
    match &MENU[3] {
        MenuEntry::Item { id, label, action } => {
            assert_eq!((*id, *label), ("quit", "Quit ClickClak"));
            assert!(matches!(action, TrayAction::Quit));
        }
        _ => panic!("last entry must be Quit"),
    }
}

#[test]
fn ac2_each_id_maps_to_one_action_and_an_unknown_id_to_none() {
    assert!(matches!(action_for("sound"), Some(TrayAction::ToggleSound)));
    assert!(matches!(
        action_for("settings"),
        Some(TrayAction::ShowSettings)
    ));
    assert!(matches!(action_for("quit"), Some(TrayAction::Quit)));
    assert!(action_for("nope").is_none());
    assert!(action_for("").is_none());
    assert!(action_for("Sound").is_none());
}

// AC3: toggling from the tray

#[test]
fn ac3_sound_on_then_chosen_turns_off_in_force_and_in_the_file_and_keeps_other_fields() {
    let rig = rig(Some(&file("full")), &PACKS);
    let gate = gate_with(&rig.core);
    let (buffer, err) = fake_err();

    rig.player.handle(down(KeyCode::KeyA));
    assert_eq!(rig.sink.count(), 1);

    let result = toggle_sound(&gate, &err);
    assert!(!enabled_of(&result));
    assert_eq!(result["saved"], true);
    assert!(!rig.live.enabled());
    let s = rig.settings();
    assert!(!s.enabled);
    assert_eq!(s.pack_id, "full");
    assert_eq!(s.volume, 0.8);
    assert_eq!(s.pitch_variation, 0.2);
    assert!(!s.key_up_sounds);
    assert!(s.launch_at_login);

    for code in [KeyCode::KeyA, KeyCode::Space, KeyCode::ShiftLeft] {
        rig.player.handle(down(code));
        rig.player.handle(up(code));
    }
    assert_eq!(rig.sink.count(), 1, "nothing plays while off");
    assert_eq!(written(&buffer), "", "a good save writes nothing");
}

#[test]
fn ac3_chosen_again_turns_on_and_the_next_key_plays() {
    let rig = rig(Some(&file("full")), &PACKS);
    let gate = gate_with(&rig.core);
    let (_, err) = fake_err();
    toggle_sound(&gate, &err);
    let result = toggle_sound(&gate, &err);
    assert!(enabled_of(&result));
    assert!(rig.live.enabled());
    assert!(rig.settings().enabled);
    rig.player.handle(down(KeyCode::KeyA));
    assert_eq!(rig.sink.count(), 1);
}

#[test]
fn ac3_core_toggle_enabled_flips_and_returns_the_set_settings_shape() {
    let rig = rig(Some(&file("full")), &PACKS);
    let v = rig.core.toggle_enabled();
    assert!(!enabled_of(&v));
    assert_eq!(v["saved"], true);
    assert!(v["save_error"].is_null());
    assert_eq!(v["playing_pack"], "full");
    assert!(v["revision"].is_u64());
    assert!(enabled_of(&rig.core.toggle_enabled()));
}

// AC4: a failed save

#[test]
fn ac4_a_failed_save_keeps_the_change_reports_it_and_writes_one_line() {
    let (core, live, player, sink, _dir, _packs) = blocked_rig();
    let (seen, listener) = recorder();
    core.subscribe(listener);
    let gate = gate_with(&core);
    let (buffer, err) = fake_err();

    let result = toggle_sound(&gate, &err);
    assert!(!enabled_of(&result), "the change is in force");
    assert_eq!(result["saved"], false);
    let reason = result["save_error"].as_str().unwrap().to_string();
    assert!(!reason.is_empty());
    assert!(!live.enabled());
    assert_eq!(core.get_settings()["settings"]["enabled"], false);
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink.count(), 0);

    // The window is told, with the failure.
    let last = seen.lock().unwrap().last().unwrap().clone();
    assert_eq!(last["saved"], false);
    assert_eq!(last["save_error"], reason.as_str());
    assert!(!enabled_of(&last));
    // The tray shows the new state.
    let sync = TraySync::new();
    sync.update(&seen.lock().unwrap()[0]);
    let view = sync.update(&last).expect("the tray view changed");
    assert!(!view.checked);

    let out = written(&buffer);
    assert_eq!(out.lines().count(), 1, "exactly one line: {out:?}");
    assert!(out.starts_with("settings: could not save: "), "{out:?}");
    assert!(out.contains(&reason), "{out:?}");
}

// AC5: both directions are announced

#[test]
fn ac5_a_subscriber_gets_set_settings_and_toggle_results_with_rising_revisions() {
    let rig = rig(Some(&file("full")), &PACKS);
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);

    let a = rig.core.set_settings(&json!({"enabled": false})).unwrap();
    let b = rig.core.toggle_enabled();
    let c = rig.core.set_settings(&json!({"volume": 0.5})).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 4, "the initial call and one per change");
    assert_eq!(seen[1], a);
    assert_eq!(seen[2], b);
    assert_eq!(seen[3], c);
    let revisions: Vec<u64> = seen
        .iter()
        .map(|v| v["revision"].as_u64().unwrap())
        .collect();
    assert_eq!(revisions, [0, 1, 2, 3]);
    assert_eq!(rig.core.get_settings()["revision"], 3);
}

#[test]
fn ac5_the_tray_view_follows_changes_from_either_side() {
    let rig = rig(Some(&file("full")), &PACKS);
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);
    let sync = TraySync::new();

    rig.core.set_settings(&json!({"enabled": false})).unwrap();
    rig.core.toggle_enabled();
    rig.core.toggle_enabled();

    let seen = seen.lock().unwrap();
    let mut views = Vec::new();
    for v in seen.iter() {
        if let Some(view) = sync.update(v) {
            views.push(view);
        }
    }
    // on (initial), off (window), on (tray), off (tray)
    let ticks: Vec<bool> = views.iter().map(|v| v.checked).collect();
    assert_eq!(ticks, [true, false, true, false]);
    assert_eq!(views[1], TrayView::of(false));
    assert_eq!(views[2], TrayView::of(true));
}

// AC6: volume and pack changes leave the tray alone

#[test]
fn ac6_a_volume_only_or_pack_only_change_gives_no_new_view() {
    let rig = rig(Some(&file("full")), &PACKS);
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);
    let sync = TraySync::new();
    assert!(sync.update(&seen.lock().unwrap()[0]).is_some());

    let volume = rig.core.set_settings(&json!({"volume": 0.3})).unwrap();
    assert!(sync.update(&volume).is_none());
    let pack = rig.core.set_settings(&json!({"pack_id": "plain"})).unwrap();
    assert!(sync.update(&pack).is_none());
    let both = rig
        .core
        .set_settings(&json!({"volume": 0.9, "pack_id": "full"}))
        .unwrap();
    assert!(sync.update(&both).is_none());

    let off = rig.core.set_settings(&json!({"enabled": false})).unwrap();
    assert_eq!(sync.update(&off), Some(TrayView::of(false)));
    let again = rig.core.set_settings(&json!({"volume": 0.1})).unwrap();
    assert!(sync.update(&again).is_none());
}

#[test]
fn ac6_setting_enabled_to_the_value_it_has_gives_no_new_view() {
    let rig = rig(Some(&file("full")), &PACKS);
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);
    let sync = TraySync::new();
    sync.update(&seen.lock().unwrap()[0]);
    let same = rig.core.set_settings(&json!({"enabled": true})).unwrap();
    assert!(sync.update(&same).is_none());
}

// AC7: before the core is published

#[test]
fn ac7_a_toggle_before_the_gate_opens_applies_once_to_the_loaded_value() {
    let rig = rig(
        Some(&file("full").replace("\"enabled\": true", "\"enabled\": false")),
        &PACKS,
    );
    let gate = Arc::new(CoreGate::new());
    let (_, err) = fake_err();
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);

    let worker = {
        let (gate, err) = (Arc::clone(&gate), err.clone());
        std::thread::spawn(move || toggle_sound(&gate, &err))
    };
    std::thread::sleep(Duration::from_millis(100));
    assert!(!worker.is_finished(), "waits for the core");
    assert_eq!(seen.lock().unwrap().len(), 1, "nothing applied yet");

    gate.publish(Arc::clone(&rig.core));
    let result = worker.join().unwrap();
    assert!(enabled_of(&result), "flipped from the loaded false");
    assert!(rig.live.enabled());
    assert!(rig.settings().enabled);
    assert_eq!(seen.lock().unwrap().len(), 2, "applied once");
    assert_eq!(rig.core.get_settings()["revision"], 1);
}

#[test]
fn ac7_two_toggles_before_the_gate_opens_leave_enabled_as_loaded() {
    let rig = rig(Some(&file("full")), &PACKS);
    let gate = Arc::new(CoreGate::new());
    let (_, err) = fake_err();
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let (gate, err) = (Arc::clone(&gate), err.clone());
            std::thread::spawn(move || toggle_sound(&gate, &err))
        })
        .collect();
    std::thread::sleep(Duration::from_millis(100));
    gate.publish(Arc::clone(&rig.core));
    for w in workers {
        w.join().unwrap();
    }
    assert!(rig.live.enabled(), "as loaded (true)");
    assert!(rig.settings().enabled);
    assert_eq!(rig.core.get_settings()["revision"], 2);
}

// AC8 and AC9: close and exit decisions

#[test]
fn ac8_a_close_request_hides_when_the_tray_exists_and_closes_when_it_does_not() {
    assert!(matches!(on_close_request(true), CloseDecision::Hide));
    assert!(matches!(on_close_request(false), CloseDecision::Close));
}

#[test]
fn ac9_exit_is_refused_only_when_no_quit_was_asked_and_the_tray_exists() {
    assert!(matches!(on_exit_request(false, true), ExitDecision::Stay));
    assert!(matches!(on_exit_request(true, true), ExitDecision::Exit));
    assert!(matches!(on_exit_request(true, false), ExitDecision::Exit));
    assert!(matches!(on_exit_request(false, false), ExitDecision::Exit));
}

// AC10: the key path

#[test]
fn ac10_keys_play_without_waiting_while_a_toggle_is_stuck_in_a_slow_subscriber() {
    let rig = rig(Some(&file("full")), &PACKS);
    let blocking = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(AtomicBool::new(false));
    {
        let (blocking, entered) = (Arc::clone(&blocking), Arc::clone(&entered));
        rig.core.subscribe(Box::new(move |_| {
            if blocking.load(Ordering::SeqCst) {
                entered.store(true, Ordering::SeqCst);
                while blocking.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }));
    }
    blocking.store(true, Ordering::SeqCst);
    let toggler = {
        let core = Arc::clone(&rig.core);
        std::thread::spawn(move || core.toggle_enabled())
    };
    let start = Instant::now();
    while !entered.load(Ordering::SeqCst) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "listener never ran"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // The toggle is in progress; the change is already in force (off), so
    // turn the sound back on through Live to prove the key path is free.
    rig.live.set_enabled(true);
    let mut slowest = Duration::ZERO;
    for _ in 0..500 {
        let t = Instant::now();
        rig.player.handle(down(KeyCode::KeyA));
        slowest = slowest.max(t.elapsed());
    }
    assert_eq!(rig.sink.count(), 500);
    assert!(slowest < Duration::from_millis(100), "slowest {slowest:?}");
    assert!(!toggler.is_finished(), "the toggle is still held up");
    blocking.store(false, Ordering::SeqCst);
    toggler.join().unwrap();
}

// Edge cases

#[test]
fn edge_subscribe_calls_the_listener_once_at_once_with_the_current_state() {
    let rig = rig(Some(&file("full")), &PACKS);
    rig.core.set_settings(&json!({"volume": 0.4})).unwrap();
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["settings"]["volume"], 0.4);
    assert_eq!(seen[0]["playing_pack"], "full");
    assert_eq!(seen[0]["saved"], true);
    assert!(seen[0]["save_error"].is_null());
    assert_eq!(seen[0]["revision"], 1);
}

#[test]
fn edge_a_rejected_patch_notifies_nobody_and_does_not_raise_the_revision() {
    let rig = rig(Some(&file("full")), &PACKS);
    let (seen, listener) = recorder();
    rig.core.subscribe(listener);
    assert!(rig.core.set_settings(&json!({"volume": 2.0})).is_err());
    assert!(rig.core.set_settings(&json!({"pack_id": "nope"})).is_err());
    assert_eq!(seen.lock().unwrap().len(), 1, "only the initial call");
    assert_eq!(rig.core.get_settings()["revision"], 0);
    let ok = rig.core.set_settings(&json!({"volume": 0.5})).unwrap();
    assert_eq!(ok["revision"], 1);
}

#[test]
fn edge_get_settings_starts_at_revision_zero() {
    let rig = rig(Some(&file("full")), &PACKS);
    assert_eq!(rig.core.get_settings()["revision"], 0);
}

#[test]
fn edge_listeners_run_in_subscription_order() {
    let rig = rig(Some(&file("full")), &PACKS);
    let order = Arc::new(Mutex::new(Vec::new()));
    for name in ["a", "b"] {
        let order = Arc::clone(&order);
        rig.core.subscribe(Box::new(move |v| {
            order
                .lock()
                .unwrap()
                .push((name, v["revision"].as_u64().unwrap()))
        }));
    }
    order.lock().unwrap().clear();
    rig.core.toggle_enabled();
    assert_eq!(*order.lock().unwrap(), [("a", 1), ("b", 1)]);
}

#[test]
fn edge_a_listener_that_panics_does_not_poison_later_changes() {
    let rig = rig(Some(&file("full")), &PACKS);
    let armed = Arc::new(AtomicBool::new(false));
    {
        let armed = Arc::clone(&armed);
        rig.core.subscribe(Box::new(move |_| {
            if armed.swap(false, Ordering::SeqCst) {
                panic!("a bad listener");
            }
        }));
    }
    armed.store(true, Ordering::SeqCst);
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rig.core.set_settings(&json!({"volume": 0.2}))
    }));
    // Whether the panic is contained or propagates, the change is in force.
    drop(first);
    assert_eq!(rig.core.get_settings()["settings"]["volume"], 0.2);
    let next = rig.core.set_settings(&json!({"volume": 0.6})).unwrap();
    assert_eq!(next["settings"]["volume"], 0.6);
    assert!(next["revision"].as_u64().unwrap() >= 2);
    assert!(!enabled_of(&rig.core.toggle_enabled()));
}
