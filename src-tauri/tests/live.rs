//! Tests for the live state on the key path (feature 06: AC17, AC18, AC19,
//! AC20, AC23). A `Player` over a `Live` and a recording sink; packs are
//! switched through `Core::set_settings`. The fixture pack `full` plays space
//! at 70 ms and letters at 40 to 50 ms; `plain` plays 40 to 50 ms for every
//! key, so a space at 70 ms is pack A and anything shorter is pack B.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clickclak_lib::audio::{Live, Player, Sample, SoundSet};
use clickclak_lib::keys::KeyCode;
use clickclak_lib::packs::load;
use clickclak_lib::startup::{Wait, key_consumer, run_live};
use serde_json::json;

mod common;
use common::*;
mod support;
use support::*;

const PACKS: [(&str, &str); 2] = [("full", "full"), ("plain", "default-only")];

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn live_for(pack: &str) -> Arc<Live> {
    Arc::new(Live::new(load(&fixture_pack(pack)).unwrap().sounds))
}

// Gain on a sample

#[test]
fn ac18_a_new_sample_has_gain_one_and_with_gain_shares_the_buffer() {
    let s = Sample::new(1, 48_000, vec![0.1, 0.2, 0.3]);
    assert_eq!(s.gain(), 1.0);
    let quiet = s.with_gain(0.25);
    assert_eq!(quiet.gain(), 0.25);
    assert!(quiet.same_buffer(&s), "nothing is copied");
    assert_eq!(quiet.samples(), s.samples(), "the data is untouched");
    // Not multiplied with the old gain.
    assert_eq!(quiet.with_gain(0.5).gain(), 0.5);
    assert_eq!(s.gain(), 1.0, "the original keeps its gain");
}

// AC17: sound on and off

#[test]
fn ac17_nothing_plays_while_off_and_the_next_key_plays_when_back_on() {
    let live = live_for("full");
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));

    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink.count(), 1);

    live.set_enabled(false);
    assert!(!live.enabled());
    for code in [KeyCode::KeyA, KeyCode::Space, KeyCode::ShiftLeft] {
        player.handle(down(code));
        player.handle(up(code));
    }
    assert_eq!(sink.count(), 1, "nothing played while off");

    live.set_enabled(true);
    assert!(live.enabled());
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink.count(), 2);
}

#[test]
fn ac17_a_new_live_is_enabled_at_full_gain() {
    let live = live_for("full");
    assert!(live.enabled());
    assert_eq!(live.gain(), 1.0);
}

// AC18: volume is squared

#[test]
fn ac18_the_gain_is_the_volume_squared() {
    let live = live_for("full");
    for (v, gain) in [
        (1.0, 1.0_f32),
        (0.0, 0.0),
        (0.5, 0.25),
        (0.8, 0.64),
        (0.1, 0.01),
    ] {
        live.set_volume(v);
        assert!(
            approx(live.gain(), gain),
            "volume {v}: gain {}",
            live.gain()
        );
    }
}

#[test]
fn ac18_the_next_key_plays_with_the_gain_and_earlier_sounds_keep_theirs() {
    let live = live_for("full");
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));

    player.handle(down(KeyCode::Space));
    live.set_volume(0.5);
    player.handle(down(KeyCode::Space));
    live.set_volume(0.0);
    player.handle(down(KeyCode::Space));
    live.set_volume(1.0);
    player.handle(down(KeyCode::Space));

    let played = sink.samples();
    assert_eq!(played.len(), 4);
    assert_eq!(played[0].gain(), 1.0, "the first sound was not changed");
    assert!(approx(played[1].gain(), 0.25));
    assert_eq!(played[2].gain(), 0.0);
    assert_eq!(played[3].gain(), 1.0, "100% plays the samples as they are");
    // Same audio, only the gain differs; the pack's buffer is shared.
    assert_eq!(played[1].samples(), played[0].samples());
    assert!(played[1].same_buffer(&played[0]));
}

#[test]
fn ac18_with_sounds_and_new_players_still_play_at_gain_one() {
    let sink = Arc::new(RecordingSink::default());
    let p = Player::with_sounds(
        sink.clone(),
        SoundSet::single(Sample::new(1, 48_000, vec![0.5])),
    );
    p.handle(down(KeyCode::KeyA));
    let q = Player::new(sink.clone(), Sample::new(1, 48_000, vec![0.5]));
    q.handle(down(KeyCode::KeyA));
    assert!(sink.samples().iter().all(|s| s.gain() == 1.0));
}

// AC19: switching packs while keys arrive

#[test]
fn ac19_events_play_the_old_pack_until_the_new_one_is_set_and_the_new_one_after() {
    let rig = rig(Some(&file("full")), &PACKS);
    rig.player.handle(down(KeyCode::Space));
    assert_eq!(ms(&rig.sink.samples()[0]), 70);

    rig.core.set_settings(&json!({"pack_id": "plain"})).unwrap();
    rig.player.handle(down(KeyCode::Space));
    let second = ms(&rig.sink.samples()[1]);
    assert!([40, 45, 50].contains(&second), "played {second} ms");

    rig.core.set_settings(&json!({"pack_id": "full"})).unwrap();
    rig.player.handle(down(KeyCode::Space));
    assert_eq!(ms(&rig.sink.samples()[2]), 70);
}

#[test]
fn ac19_no_event_is_unplayed_or_mixed_up_during_a_switch() {
    let rig = rig(Some(&file("full")), &PACKS);
    let player = Arc::new(rig.player);
    let stop = Arc::new(AtomicBool::new(false));
    let sent = Arc::new(Mutex::new(0usize));

    let hammer = {
        let (player, stop, sent) = (Arc::clone(&player), Arc::clone(&stop), Arc::clone(&sent));
        std::thread::spawn(move || {
            let mut n = 0;
            while !stop.load(Ordering::Relaxed) {
                player.handle(down(KeyCode::Space));
                n += 1;
            }
            *sent.lock().unwrap() = n;
        })
    };
    std::thread::sleep(Duration::from_millis(20));
    rig.core.set_settings(&json!({"pack_id": "plain"})).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    stop.store(true, Ordering::Relaxed);
    hammer.join().unwrap();

    let lengths: Vec<u64> = rig.sink.samples().iter().map(ms).collect();
    assert_eq!(lengths.len(), *sent.lock().unwrap(), "every event played");
    assert!(
        lengths.iter().all(|l| [70, 40, 45, 50].contains(l)),
        "{lengths:?}"
    );
    // Once the new pack is heard the old one never returns.
    let first_b = lengths.iter().position(|l| *l != 70).expect("B was heard");
    assert!(lengths[first_b..].iter().all(|l| *l != 70));
    assert!(first_b > 0, "A played first");
}

#[test]
fn ac19_set_sounds_while_the_key_thread_is_mid_event_loses_no_event() {
    let live = live_for("full");
    let sink = Arc::new(RecordingSink::default());
    let player = Arc::new(Player::with_live(sink.clone(), Arc::clone(&live)));
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (live, stop) = (Arc::clone(&live), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                live.set_sounds(load(&fixture_pack("default-only")).unwrap().sounds);
                live.set_sounds(load(&fixture_pack("full")).unwrap().sounds);
            }
        })
    };
    for _ in 0..2000 {
        player.handle(down(KeyCode::KeyA));
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
    assert_eq!(sink.count(), 2000);
}

// AC20: run_live

#[test]
fn ac20_run_live_plays_from_the_live_state_from_the_first_event() {
    let live = live_for("full");
    live.set_volume(0.5);
    live.set_enabled(false);
    let (_, err) = fake_err();
    let sink = Arc::new(RecordingSink::default());
    let l = Arc::clone(&live);
    run_live(
        move |mut consumer| {
            consumer(down(KeyCode::KeyA));
            l.set_enabled(true);
            consumer(down(KeyCode::KeyA));
            Ok(Box::new(|| Ok(())) as Wait)
        },
        sink.clone(),
        live,
        err,
    );
    let played = sink.samples();
    assert_eq!(
        played.len(),
        1,
        "the first event was silent, the second played"
    );
    assert!(approx(played[0].gain(), 0.25));
}

// AC23: the key path

#[test]
fn ac23_key_events_are_handled_while_settings_are_changed_and_saved() {
    let rig = rig(Some(&file("full")), &PACKS);
    let player = Arc::new(rig.player);
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let (core, stop) = (Arc::clone(&rig.core), Arc::clone(&stop));
        std::thread::spawn(move || {
            let mut i = 0;
            while !stop.load(Ordering::Relaxed) {
                let id = if i % 2 == 0 { "plain" } else { "full" };
                core.set_settings(&json!({"pack_id": id, "volume": (i % 10) as f64 / 10.0}))
                    .unwrap();
                i += 1;
            }
        })
    };
    let mut slowest = Duration::ZERO;
    for _ in 0..3000 {
        let start = Instant::now();
        player.handle(down(KeyCode::KeyA));
        slowest = slowest.max(start.elapsed());
    }
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    assert_eq!(rig.sink.count(), 3000);
    assert!(
        slowest < Duration::from_millis(100),
        "slowest event {slowest:?}"
    );
}

#[test]
fn ac23_nothing_from_key_codes_is_written_to_errors_or_the_settings_file() {
    let rig = rig(Some(&file("full")), &PACKS);
    let (buffer, err) = fake_err();
    let Rig {
        player,
        core,
        sink,
        settings_path,
        ..
    } = rig;
    let mut consumer = key_consumer(player, err);
    for code in [
        KeyCode::KeyQ,
        KeyCode::Digit7,
        KeyCode::Space,
        KeyCode::Unknown(777),
    ] {
        consumer(down(code));
        consumer(up(code));
        core.set_settings(&json!({"volume": 0.4})).unwrap();
    }
    assert_eq!(written(&buffer), "", "no output per key");
    let text = std::fs::read_to_string(&settings_path).unwrap();
    for needle in ["KeyQ", "Digit7", "Space", "777", "Unknown"] {
        assert!(!text.contains(needle), "{needle} in the settings file");
    }
    // The settings file has `key_up_sounds: false`, which feature 09 puts in
    // force from the first key event: the four releases play nothing and
    // only the four presses play.
    assert_eq!(sink.count(), 4);
}
