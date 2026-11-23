//! Tests for sound tuning on the key path (feature 09: AC12, AC13, AC15,
//! AC23). A `Player` over a `Live` and a recording sink.
//!
//! Interfaces assumed from the feature 09 plan:
//!
//! ```ignore
//! Sample::speed(&self) -> f32; Sample::tuned(&self, gain: f32, speed: f32) -> Sample
//! Live::set_pitch_variation(f64); pitch_variation() -> f32; set_key_up(bool); key_up() -> bool
//! SoundSet::random(&self) -> u32
//! audio::voice(&Live, &SoundSet, KeyKind, KeyGroup) -> Option<Sample>
//! audio::speed_for(variation: f32, random: u32) -> f32
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use mechanical_key_sounds_lib::audio::{
    Live, Player, Previewer, Sample, Sink, SinkSlot, SoundSet, speed_for, voice,
};
use mechanical_key_sounds_lib::keys::{KeyCode, KeyGroup, KeyKind};
use mechanical_key_sounds_lib::packs::load;

mod common;
use common::*;
mod support;
use support::*;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

/// A sample whose data says which one it is.
fn tagged(tag: f32, frames: usize) -> Sample {
    Sample::new(1, 1000, vec![tag; frames])
}

fn two_down_one_up(seed: u32) -> SoundSet {
    let mut down: [Vec<Sample>; 5] = Default::default();
    down[KeyGroup::Default.index()] = vec![tagged(0.1, 10), tagged(0.2, 10), tagged(0.3, 10)];
    let mut up: [Vec<Sample>; 5] = Default::default();
    up[KeyGroup::Default.index()] = vec![tagged(0.9, 5)];
    SoundSet::with_seed(down, up, seed).unwrap()
}

fn live_with(sounds: SoundSet) -> Arc<Live> {
    Arc::new(Live::new(sounds))
}

// The speed on a sample

#[test]
fn ac12_a_new_sample_has_speed_one_and_tuned_sets_gain_and_speed_sharing_the_buffer() {
    let s = Sample::new(1, 48_000, vec![0.1, 0.2, 0.3]);
    assert_eq!(s.speed(), 1.0);
    let t = s.tuned(0.25, 1.2);
    assert_eq!(t.gain(), 0.25);
    assert!(approx(t.speed(), 1.2));
    assert!(t.same_buffer(&s), "nothing is copied");
    assert_eq!(t.samples(), s.samples());
    assert_eq!(t.duration(), s.duration(), "duration is at normal speed");
    assert_eq!(s.speed(), 1.0, "the original is not changed");
}

#[test]
fn ac12_with_gain_keeps_the_speed() {
    let s = Sample::new(1, 48_000, vec![0.1; 4]).tuned(1.0, 0.8);
    let g = s.with_gain(0.5);
    assert_eq!(g.gain(), 0.5);
    assert!(approx(g.speed(), 0.8));
}

#[test]
fn ac12_a_speed_that_is_not_finite_or_not_above_zero_is_stored_as_one() {
    let s = Sample::new(1, 48_000, vec![0.1; 4]);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -0.5] {
        assert_eq!(s.tuned(1.0, bad).speed(), 1.0, "speed {bad}");
    }
}

// The live state

#[test]
fn ac12_a_new_live_has_no_pitch_variation_and_key_up_on() {
    let live = live_with(two_down_one_up(1));
    assert_eq!(live.pitch_variation(), 0.0);
    assert!(live.key_up());
}

#[test]
fn ac12_pitch_variation_is_clamped_to_zero_to_point_three_and_nan_is_zero() {
    let live = live_with(two_down_one_up(1));
    live.set_pitch_variation(0.12);
    assert!(approx(live.pitch_variation(), 0.12));
    live.set_pitch_variation(0.3);
    assert!(approx(live.pitch_variation(), 0.3));
    live.set_pitch_variation(5.0);
    assert!(approx(live.pitch_variation(), 0.3));
    live.set_pitch_variation(-1.0);
    assert_eq!(live.pitch_variation(), 0.0);
    live.set_pitch_variation(0.2);
    live.set_pitch_variation(f64::NAN);
    assert_eq!(live.pitch_variation(), 0.0);
}

#[test]
fn ac15_key_up_can_be_turned_off_and_on() {
    let live = live_with(two_down_one_up(1));
    live.set_key_up(false);
    assert!(!live.key_up());
    live.set_key_up(true);
    assert!(live.key_up());
}

// speed_for

#[test]
fn ac13_speed_for_zero_variation_is_exactly_one_whatever_the_random_number() {
    for r in [0, 1, 0x8000_0000, 0xFFFF_FFFF, 12345] {
        assert_eq!(speed_for(0.0, r), 1.0);
    }
}

#[test]
fn ac12_speed_for_spans_one_minus_p_up_to_but_not_including_one_plus_p() {
    let p = 0.2_f32;
    assert!(approx(speed_for(p, 0), 0.8));
    assert!(approx(speed_for(p, 0x8000_0000), 1.0));
    let top = speed_for(p, u32::MAX);
    assert!(top < 1.2 && top > 1.19999, "top {top}");
    // Monotone in the random number.
    let mut last = 0.0;
    for r in (0..=u32::MAX).step_by(0x0100_0000) {
        let f = speed_for(p, r);
        assert!(f >= last);
        last = f;
    }
}

// AC12: the factors the player hands the sink

#[test]
fn ac12_each_sound_gets_a_factor_in_range_consecutive_ones_differ_and_they_spread() {
    let p = 0.2_f32;
    let live = live_with(two_down_one_up(7));
    live.set_pitch_variation(f64::from(p));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    for _ in 0..1000 {
        player.handle(down(KeyCode::KeyA));
    }
    let factors: Vec<f32> = sink.samples().iter().map(Sample::speed).collect();
    assert_eq!(factors.len(), 1000);
    for f in &factors {
        assert!(*f >= 1.0 - p - 1e-6 && *f <= 1.0 + p + 1e-6, "factor {f}");
    }
    for pair in factors.windows(2) {
        assert_ne!(pair[0], pair[1], "consecutive factors differ");
    }
    let min = factors.iter().cloned().fold(f32::MAX, f32::min);
    let max = factors.iter().cloned().fold(f32::MIN, f32::max);
    assert!(min < 1.0 - p / 2.0, "min {min}");
    assert!(max > 1.0 + p / 2.0, "max {max}");
}

#[test]
fn ac12_key_up_sounds_are_varied_too_and_the_shared_buffer_is_not_copied() {
    let live = live_with(two_down_one_up(7));
    live.set_pitch_variation(0.3);
    let original = live.snapshot();
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    for _ in 0..20 {
        player.handle(down(KeyCode::KeyA));
        player.handle(up(KeyCode::KeyA));
    }
    let played = sink.samples();
    let ups: Vec<&Sample> = played.iter().filter(|s| s.samples()[0] == 0.9).collect();
    assert_eq!(ups.len(), 20);
    assert!(ups.iter().any(|s| s.speed() != 1.0));
    let up_original = original.pick(KeyKind::Up, KeyGroup::Default).unwrap();
    for s in ups {
        assert!(s.same_buffer(up_original), "the buffer is shared");
    }
}

#[test]
fn ac12_the_gain_is_still_applied_with_the_speed() {
    let live = live_with(two_down_one_up(7));
    live.set_volume(0.5);
    live.set_pitch_variation(0.1);
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    player.handle(down(KeyCode::KeyA));
    assert!(approx(sink.samples()[0].gain(), 0.25));
}

#[test]
fn ac12_a_change_of_pitch_is_heard_from_the_next_sound_and_earlier_ones_keep_theirs() {
    let live = live_with(two_down_one_up(7));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    player.handle(down(KeyCode::KeyA));
    live.set_pitch_variation(0.3);
    for _ in 0..50 {
        player.handle(down(KeyCode::KeyA));
    }
    let played = sink.samples();
    assert_eq!(played[0].speed(), 1.0, "the first sound was not changed");
    assert!(played[1..].iter().any(|s| s.speed() != 1.0));
}

// AC13: pitch variation 0

#[test]
fn ac13_with_pitch_off_every_factor_is_exactly_one_and_the_samples_are_untouched() {
    let live = live_with(two_down_one_up(7));
    let original = live.snapshot();
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    for _ in 0..100 {
        player.handle(down(KeyCode::KeyA));
        player.handle(up(KeyCode::KeyA));
    }
    let played = sink.samples();
    assert_eq!(played.len(), 200);
    for s in &played {
        assert_eq!(s.speed(), 1.0);
        assert_eq!(s.gain(), 1.0);
    }
    for s in played.iter().filter(|s| s.samples()[0] != 0.9) {
        assert!(
            [0.1_f32, 0.2, 0.3].contains(&s.samples()[0]),
            "a sample of the pack"
        );
    }
    let up_original = original.pick(KeyKind::Up, KeyGroup::Default).unwrap();
    assert_eq!(up_original.speed(), 1.0);
}

#[test]
fn ac13_with_pitch_off_the_pick_sequence_is_what_it_was_without_the_feature() {
    // Two sets with the same seed: one picked through the player with pitch
    // off, one picked directly. No random number may be spent on pitch.
    let live = live_with(two_down_one_up(99));
    let reference = two_down_one_up(99);
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    for _ in 0..200 {
        player.handle(down(KeyCode::KeyA));
    }
    let played: Vec<f32> = sink.samples().iter().map(|s| s.samples()[0]).collect();
    let expected: Vec<f32> = (0..200)
        .map(|_| {
            reference
                .pick(KeyKind::Down, KeyGroup::Default)
                .unwrap()
                .samples()[0]
        })
        .collect();
    assert_eq!(played, expected);
}

#[test]
fn ac13_a_pack_with_one_sample_per_group_and_pitch_off_still_plays() {
    let live = live_with(SoundSet::single(tagged(0.5, 10)));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    player.handle(down(KeyCode::KeyA));
    player.handle(down(KeyCode::Space));
    assert_eq!(sink.count(), 2);
    assert!(sink.samples().iter().all(|s| s.speed() == 1.0));
}

#[test]
fn ac12_voice_gives_the_sample_with_gain_and_speed_and_none_for_a_silent_group() {
    let live = live_with(two_down_one_up(3));
    live.set_volume(0.5);
    live.set_pitch_variation(0.1);
    let set = live.snapshot();
    let s = voice(&live, &set, KeyKind::Down, KeyGroup::Default).unwrap();
    assert!(approx(s.gain(), 0.25));
    assert!(s.speed() >= 0.9 - 1e-6 && s.speed() <= 1.1 + 1e-6);

    let click = SoundSet::single(tagged(0.5, 10));
    assert!(voice(&live, &click, KeyKind::Up, KeyGroup::Default).is_none());
}

// AC15: key-up sounds off

#[test]
fn ac15_with_key_up_off_a_release_plays_nothing_and_presses_still_play() {
    let live = Arc::new(Live::new(load(&fixture_pack("full")).unwrap().sounds));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));

    player.handle(up(KeyCode::KeyA));
    assert_eq!(sink.count(), 1, "on: the release plays");

    live.set_key_up(false);
    for code in [KeyCode::KeyA, KeyCode::Space] {
        player.handle(up(code));
    }
    assert_eq!(sink.count(), 1, "off: releases are silent");
    player.handle(down(KeyCode::KeyA));
    player.handle(down(KeyCode::Space));
    assert_eq!(sink.count(), 3, "key-down sounds are unaffected");

    live.set_key_up(true);
    player.handle(up(KeyCode::Space));
    assert_eq!(sink.count(), 4, "back on: the next release plays");
}

#[test]
fn ac15_a_release_while_off_does_not_touch_the_up_lists_last_chosen_memory() {
    // Three up samples in one group; with "never the same twice in a row"
    // the sequence must be the same whether or not silent releases came in
    // between.
    let build = || {
        let mut down: [Vec<Sample>; 5] = Default::default();
        down[0] = vec![tagged(0.1, 10)];
        let mut up: [Vec<Sample>; 5] = Default::default();
        up[0] = vec![tagged(0.5, 5), tagged(0.6, 5), tagged(0.7, 5)];
        SoundSet::with_seed(down, up, 5).unwrap()
    };
    let run = |silent_releases: bool| {
        let live = live_with(build());
        let sink = Arc::new(RecordingSink::default());
        let player = Player::with_live(sink.clone(), Arc::clone(&live));
        for _ in 0..30 {
            if silent_releases {
                live.set_key_up(false);
                player.handle(up(KeyCode::KeyA));
                player.handle(up(KeyCode::KeyA));
                live.set_key_up(true);
            }
            player.handle(up(KeyCode::KeyA));
        }
        sink.samples()
            .iter()
            .map(|s| s.samples()[0])
            .collect::<Vec<f32>>()
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn ac15_a_pack_without_up_samples_plays_nothing_on_release_either_way() {
    let live = live_with(SoundSet::single(tagged(0.5, 10)));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    player.handle(up(KeyCode::KeyA));
    assert_eq!(sink.count(), 0);
}

// AC23: the key path is not held up

#[test]
fn ac23_key_events_play_while_another_thread_keeps_changing_settings() {
    let rig = rig(
        Some(&file("full")),
        &[("full", "full"), ("plain", "default-only")],
    );
    let core = Arc::clone(&rig.core);
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let worker = std::thread::spawn(move || {
        let mut i = 0;
        while !flag.load(Ordering::Relaxed) {
            let pack = if i % 2 == 0 { "plain" } else { "full" };
            let _ = core.set_settings(&serde_json::json!({
                "pack_id": pack,
                "pitch_variation": (i % 30) as f64 / 100.0,
                "key_up_sounds": i % 3 == 0,
            }));
            i += 1;
        }
    });
    let start = Instant::now();
    let mut played_before = 0;
    for _ in 0..300 {
        rig.player.handle(down(KeyCode::KeyA));
        played_before += 1;
        assert_eq!(rig.sink.count(), played_before, "every press plays");
    }
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(10));
}

/// A sink that holds up every call made on a thread named `preview` until
/// released, and records what it was given.
#[derive(Default)]
struct HoldingSink {
    gate: Mutex<bool>,
    open: Condvar,
    played: Mutex<Vec<Sample>>,
}

impl HoldingSink {
    fn release(&self) {
        *self.gate.lock().unwrap() = true;
        self.open.notify_all();
    }
    fn count(&self) -> usize {
        self.played.lock().unwrap().len()
    }
}

impl Sink for HoldingSink {
    fn play(&self, sample: &Sample) {
        self.played.lock().unwrap().push(sample.clone());
        if std::thread::current().name() == Some("preview") {
            let mut open = self.gate.lock().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !*open {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return;
                }
                open = self.open.wait_timeout(open, left).unwrap().0;
            }
        }
    }
}

#[test]
fn ac23_a_key_event_plays_at_once_while_a_preview_is_held_mid_phrase() {
    let sink = Arc::new(HoldingSink::default());
    let slot = Arc::new(SinkSlot::new());
    slot.set(sink.clone());
    let live = live_with(two_down_one_up(1));
    let previewer = Previewer::new(slot.clone());
    let player = Player::with_live(slot.clone(), Arc::clone(&live));

    previewer.start(Arc::clone(&live)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while sink.count() < 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(sink.count(), 1, "the preview is inside its first sound");

    let start = Instant::now();
    player.handle(down(KeyCode::KeyA));
    assert!(start.elapsed() < Duration::from_millis(200), "no wait");
    assert_eq!(sink.count(), 2, "the key press was played, not dropped");
    sink.release();
}

/// A sink that waits inside `play` until a second thread is inside too.
#[derive(Default)]
struct MeetingSink {
    inside: Mutex<usize>,
    changed: Condvar,
    met: Mutex<usize>,
}

impl Sink for MeetingSink {
    fn play(&self, _sample: &Sample) {
        let mut inside = self.inside.lock().unwrap();
        *inside += 1;
        self.changed.notify_all();
        let deadline = Instant::now() + Duration::from_secs(3);
        while *inside < 2 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            inside = self.changed.wait_timeout(inside, left).unwrap().0;
        }
        *self.met.lock().unwrap() += 1;
    }
}

#[test]
fn ac23_the_slot_plays_from_two_threads_at_once_without_dropping_either() {
    let meeting = Arc::new(MeetingSink::default());
    let slot = Arc::new(SinkSlot::new());
    slot.set(meeting.clone());
    let a = {
        let slot = Arc::clone(&slot);
        std::thread::spawn(move || slot.play(&tagged(0.1, 4)))
    };
    let b = {
        let slot = Arc::clone(&slot);
        std::thread::spawn(move || slot.play(&tagged(0.2, 4)))
    };
    a.join().unwrap();
    b.join().unwrap();
    assert_eq!(
        *meeting.met.lock().unwrap(),
        2,
        "both were inside the sink together, so neither waited for the other"
    );
}
