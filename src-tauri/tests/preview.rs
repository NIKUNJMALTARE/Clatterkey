//! Tests for the preview (feature 09: AC19 to AC22). `play_phrase` is driven
//! with a recording sink and a `sleep` that does not sleep; `Previewer` is
//! driven with a sink that can hold the phrase.
//!
//! Interfaces assumed from the feature 09 plan, all exported from `audio`:
//!
//! ```ignore
//! pub const PHRASE: [KeyGroup; 8];  pub const HOLD: Duration;  pub const GAP: Duration;
//! pub fn play_phrase(sink: &dyn Sink, live: &Live, sounds: &SoundSet,
//!                    cancelled: impl Fn() -> bool, sleep: impl FnMut(Duration));
//! pub struct Previewer; Previewer::new(Arc<dyn Sink>);
//! Previewer::start(&self, live: Arc<Live>) -> Result<(), String>;
//! Live::snapshot(&self) -> Arc<SoundSet>;  Core::live(&self) -> Arc<Live>
//! ```
//!
//! Samples are told apart by length at 1 kHz: one frame is one millisecond.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use mechanical_key_sounds_lib::audio::{
    GAP, HOLD, Live, PHRASE, Player, Previewer, Sample, Sink, SinkSlot, SoundSet, play_phrase,
};
use mechanical_key_sounds_lib::keys::{KeyCode, KeyGroup};

mod common;
use common::*;
mod support;
use support::*;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn tone(frames: usize) -> Sample {
    Sample::new(1, 1000, vec![0.5; frames])
}

/// Down: default 10, space 20, enter 30. Up: default 11, space 21, enter 31.
fn full_set() -> SoundSet {
    let mut down: [Vec<Sample>; 5] = Default::default();
    down[KeyGroup::Default.index()] = vec![tone(10)];
    down[KeyGroup::Space.index()] = vec![tone(20)];
    down[KeyGroup::Enter.index()] = vec![tone(30)];
    let mut up: [Vec<Sample>; 5] = Default::default();
    up[KeyGroup::Default.index()] = vec![tone(11)];
    up[KeyGroup::Space.index()] = vec![tone(21)];
    up[KeyGroup::Enter.index()] = vec![tone(31)];
    SoundSet::with_seed(down, up, 3).unwrap()
}

/// The same shape with every length 100 more, to tell a swapped-in pack.
fn other_set() -> SoundSet {
    let mut down: [Vec<Sample>; 5] = Default::default();
    down[KeyGroup::Default.index()] = vec![tone(110)];
    down[KeyGroup::Space.index()] = vec![tone(120)];
    down[KeyGroup::Enter.index()] = vec![tone(130)];
    let mut up: [Vec<Sample>; 5] = Default::default();
    up[KeyGroup::Default.index()] = vec![tone(111)];
    SoundSet::with_seed(down, up, 3).unwrap()
}

fn lengths(sink: &RecordingSink) -> Vec<u64> {
    sink.samples().iter().map(ms).collect()
}

fn never() -> bool {
    false
}

fn run(live: &Live, set: &SoundSet) -> Arc<RecordingSink> {
    let sink = Arc::new(RecordingSink::default());
    play_phrase(sink.as_ref(), live, set, never, |_| {});
    sink
}

// The phrase

#[test]
fn ac19_the_phrase_is_four_letters_space_two_letters_enter() {
    use KeyGroup::*;
    assert_eq!(
        PHRASE,
        [
            Default, Default, Default, Default, Space, Default, Default, Enter
        ]
    );
}

#[test]
fn ac19_the_timing_is_about_a_second_in_all() {
    assert!(HOLD < GAP);
    let total = GAP * 8;
    assert!(
        total >= Duration::from_millis(700) && total <= Duration::from_millis(1300),
        "{total:?}"
    );
}

// AC19

#[test]
fn ac19_with_key_up_on_each_down_is_followed_by_its_up_before_the_next_down() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = run(&live, &set);
    assert_eq!(
        lengths(&sink),
        [
            10, 11, 10, 11, 10, 11, 10, 11, // four letters
            20, 21, // space
            10, 11, 10, 11, // two letters
            30, 31, // enter
        ]
    );
}

#[test]
fn ac19_with_key_up_off_only_down_sounds_play_in_order() {
    let live = Live::new(full_set());
    live.set_key_up(false);
    let set = live.snapshot();
    let sink = run(&live, &set);
    assert_eq!(lengths(&sink), [10, 10, 10, 10, 20, 10, 10, 30]);
}

#[test]
fn ac19_a_set_without_up_samples_plays_eight_downs_and_no_ups() {
    let live = Live::new(SoundSet::single(tone(10)));
    let set = live.snapshot();
    let sink = run(&live, &set);
    assert_eq!(lengths(&sink), [10; 8], "the built-in click case");
}

#[test]
fn ac19_the_sounds_carry_the_gain_and_pitch_in_force() {
    let live = Live::new(full_set());
    live.set_volume(0.5);
    live.set_pitch_variation(0.2);
    let set = live.snapshot();
    let sink = run(&live, &set);
    let played = sink.samples();
    assert_eq!(played.len(), 16);
    for s in &played {
        assert!(approx(s.gain(), 0.25));
        assert!(s.speed() >= 0.8 - 1e-6 && s.speed() <= 1.2 + 1e-6);
    }
    assert!(played.iter().any(|s| s.speed() != 1.0));
}

#[test]
fn ac19_with_pitch_off_every_factor_is_one() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = run(&live, &set);
    assert!(sink.samples().iter().all(|s| s.speed() == 1.0));
}

#[test]
fn ac19_gain_pitch_and_key_up_changed_while_it_plays_apply_to_the_remaining_keys() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = Arc::new(RecordingSink::default());
    let sleeps = Cell::new(0);
    play_phrase(sink.as_ref(), &live, &set, never, |_| {
        sleeps.set(sleeps.get() + 1);
        // After the first key (two sleeps, HOLD and the rest of GAP).
        if sleeps.get() == 2 {
            live.set_volume(0.0);
            live.set_pitch_variation(0.3);
            live.set_key_up(false);
        }
    });
    let played = sink.samples();
    // Key one in full: down and up at the old settings.
    assert_eq!(played[0].gain(), 1.0);
    assert_eq!(played[0].speed(), 1.0);
    assert_eq!(played[1].gain(), 1.0);
    // Then downs only, at the new gain.
    let rest = &played[2..];
    assert_eq!(rest.len(), 7, "key-up went off: seven downs remain");
    assert!(rest.iter().all(|s| s.gain() == 0.0));
    assert!(rest.iter().any(|s| s.speed() != 1.0));
}

#[test]
fn ac19_the_sleeps_add_up_to_one_gap_per_key_and_hold_comes_first() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let slept = RefCell::new(Vec::new());
    let sink = RecordingSink::default();
    play_phrase(&sink, &live, &set, never, |d| slept.borrow_mut().push(d));
    let slept = slept.into_inner();
    assert_eq!(slept[0], HOLD);
    let total: Duration = slept.iter().sum();
    assert_eq!(total, GAP * 8);
}

// AC20

#[test]
fn ac20_the_phrase_plays_in_full_with_sound_off() {
    let live = Live::new(full_set());
    live.set_enabled(false);
    let set = live.snapshot();
    let sink = run(&live, &set);
    assert_eq!(sink.count(), 16);
}

#[test]
fn ac20_with_sound_off_the_preview_plays_and_key_events_still_play_nothing() {
    let rig = rig(Some(&file("full")), &[("full", "full")]);
    rig.core
        .set_settings(&serde_json::json!({"enabled": false, "pitch_variation": 0.0}))
        .unwrap();
    let preview_sink = Arc::new(RecordingSink::default());
    let previewer = Previewer::new(preview_sink.clone());
    previewer.start(rig.core.live()).unwrap();
    wait_for(|| preview_sink.count() >= 8);
    assert_eq!(
        preview_sink.count(),
        8,
        "eight downs: key-up is off in the file"
    );

    rig.player.handle(down(KeyCode::KeyA));
    rig.player.handle(down(KeyCode::Space));
    assert_eq!(rig.sink.count(), 0);
}

#[test]
fn ac20_a_preview_does_not_change_settings_or_the_revision() {
    let rig = rig(Some(&file("full")), &[("full", "full")]);
    let before = rig.core.get_settings();
    let bytes = rig.bytes();
    let sink = Arc::new(RecordingSink::default());
    Previewer::new(sink.clone()).start(rig.core.live()).unwrap();
    wait_for(|| sink.count() >= 8);
    assert_eq!(rig.core.get_settings(), before);
    assert_eq!(rig.bytes(), bytes);
}

// AC21

#[test]
fn ac21_a_phrase_cancelled_before_it_starts_plays_nothing() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = RecordingSink::default();
    play_phrase(&sink, &live, &set, || true, |_| {});
    assert_eq!(sink.count(), 0);
}

#[test]
fn ac21_a_phrase_cancelled_during_the_hold_plays_no_further_sounds() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = RecordingSink::default();
    let stop = Cell::new(false);
    let sleeps = Cell::new(0);
    play_phrase(
        &sink,
        &live,
        &set,
        || stop.get(),
        |_| {
            sleeps.set(sleeps.get() + 1);
            if sleeps.get() == 1 {
                stop.set(true);
            }
        },
    );
    assert_eq!(lengths_of(&sink), [10], "the first down only, not its up");
    assert_eq!(sleeps.get(), 1, "it stops without going on to sleep");
}

#[test]
fn ac21_a_phrase_cancelled_between_keys_stops_there() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = RecordingSink::default();
    let stop = Cell::new(false);
    let sleeps = Cell::new(0);
    play_phrase(
        &sink,
        &live,
        &set,
        || stop.get(),
        |_| {
            sleeps.set(sleeps.get() + 1);
            if sleeps.get() == 2 {
                stop.set(true);
            }
        },
    );
    assert_eq!(lengths_of(&sink), [10, 11], "the first key complete");
}

fn lengths_of(sink: &RecordingSink) -> Vec<u64> {
    lengths(sink)
}

#[test]
fn ac21_a_pack_swapped_in_during_the_phrase_does_not_change_its_remaining_sounds() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let sink = RecordingSink::default();
    let sleeps = Cell::new(0);
    play_phrase(&sink, &live, &set, never, |_| {
        sleeps.set(sleeps.get() + 1);
        if sleeps.get() == 3 {
            live.set_sounds(other_set());
        }
    });
    let all = lengths_of(&sink);
    assert_eq!(all.len(), 16);
    assert!(
        all.iter().all(|l| *l < 100),
        "all from the first pack: {all:?}"
    );
}

#[test]
fn ac19_with_no_output_the_phrase_still_runs_to_its_end() {
    let live = Live::new(full_set());
    let set = live.snapshot();
    let slot = SinkSlot::new();
    let sleeps = Cell::new(0);
    play_phrase(&slot, &live, &set, never, |_| sleeps.set(sleeps.get() + 1));
    assert!(sleeps.get() >= 8);
}

// The Previewer

fn wait_for(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Records every sound and holds each one made on the `preview` thread until
/// released.
#[derive(Default)]
struct HoldingSink {
    gate: Mutex<bool>,
    open: Condvar,
    played: Mutex<Vec<u64>>,
    threads: Mutex<Vec<Option<String>>>,
}

impl HoldingSink {
    fn release(&self) {
        *self.gate.lock().unwrap() = true;
        self.open.notify_all();
    }
    fn played(&self) -> Vec<u64> {
        self.played.lock().unwrap().clone()
    }
}

impl Sink for HoldingSink {
    fn play(&self, sample: &Sample) {
        self.played.lock().unwrap().push(ms(sample));
        let name = std::thread::current().name().map(str::to_string);
        self.threads.lock().unwrap().push(name.clone());
        if name.as_deref() == Some("preview") {
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
fn ac22_start_returns_while_the_phrase_is_held_and_the_phrase_runs_off_the_calling_thread() {
    let sink = Arc::new(HoldingSink::default());
    let live = Arc::new(Live::new(full_set()));
    live.set_key_up(false);
    let previewer = Previewer::new(sink.clone());

    let began = Instant::now();
    previewer.start(Arc::clone(&live)).unwrap();
    assert!(
        began.elapsed() < Duration::from_millis(500),
        "start did not wait"
    );
    wait_for(|| !sink.played().is_empty());
    assert!(
        sink.threads
            .lock()
            .unwrap()
            .iter()
            .all(|t| t.as_deref() == Some("preview")),
        "played from a thread named preview"
    );
    sink.release();
    wait_for(|| sink.played().len() >= 8);
    assert_eq!(sink.played(), [10, 10, 10, 10, 20, 10, 10, 30]);
}

#[test]
fn ac21_a_second_start_ends_the_first_and_plays_its_own_full_phrase() {
    let sink = Arc::new(HoldingSink::default());
    let live = Arc::new(Live::new(full_set()));
    live.set_key_up(false);
    let previewer = Previewer::new(sink.clone());

    previewer.start(Arc::clone(&live)).unwrap();
    wait_for(|| sink.played().len() == 1);
    // The second start takes the set in force now.
    live.set_sounds(other_set());
    previewer.start(Arc::clone(&live)).unwrap();
    wait_for(|| sink.played().len() == 2);
    sink.release();
    wait_for(|| sink.played().len() >= 9);
    std::thread::sleep(Duration::from_millis(400));
    let played = sink.played();
    assert_eq!(
        played.len(),
        9,
        "1 from the first, 8 from the second: {played:?}"
    );
    assert_eq!(played[0], 10);
    assert_eq!(&played[1..], [110, 110, 110, 110, 120, 110, 110, 130]);
}

#[test]
fn ac21_a_pack_changed_during_a_preview_does_not_change_that_previews_sounds() {
    let sink = Arc::new(HoldingSink::default());
    let live = Arc::new(Live::new(full_set()));
    live.set_key_up(false);
    let previewer = Previewer::new(sink.clone());
    previewer.start(Arc::clone(&live)).unwrap();
    wait_for(|| sink.played().len() == 1);
    live.set_sounds(other_set());
    sink.release();
    wait_for(|| sink.played().len() >= 8);
    assert_eq!(sink.played(), [10, 10, 10, 10, 20, 10, 10, 30]);
}

#[test]
fn ac22_key_events_during_a_held_preview_play_with_no_added_delay() {
    let sink = Arc::new(HoldingSink::default());
    let slot = Arc::new(SinkSlot::new());
    slot.set(sink.clone());
    let live = Arc::new(Live::new(full_set()));
    let previewer = Previewer::new(slot.clone());
    let player = Player::with_live(slot.clone(), Arc::clone(&live));

    previewer.start(Arc::clone(&live)).unwrap();
    wait_for(|| sink.played().len() == 1);
    let began = Instant::now();
    player.handle(down(KeyCode::KeyA));
    assert!(began.elapsed() < Duration::from_millis(200));
    assert_eq!(sink.played().len(), 2);
    sink.release();
}

#[test]
fn ac19_a_previewer_over_an_empty_slot_starts_and_ends_without_a_panic() {
    let live = Arc::new(Live::new(full_set()));
    let previewer = Previewer::new(Arc::new(SinkSlot::new()));
    previewer.start(live).unwrap();
    std::thread::sleep(Duration::from_millis(50));
}
