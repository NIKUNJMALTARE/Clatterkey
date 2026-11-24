//! Tests for the `audio` module through its public interface: the built-in
//! click, the player, the output slot and the supervisor. A fake sink and a
//! fake back end stand in for the sound device, so nothing here opens one.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use clickclak_lib::audio::{
    AudioError, Backend, OutputStatus, Player, Sample, Sink, SinkSlot, Supervisor, builtin_click,
};
use clickclak_lib::keys::{KeyCode, KeyEvent, KeyKind};

/// A sink that records every sample it is asked to play.
#[derive(Default)]
struct RecordingSink {
    played: Mutex<Vec<Sample>>,
}

impl RecordingSink {
    fn count(&self) -> usize {
        self.played.lock().unwrap().len()
    }
}

impl Sink for RecordingSink {
    fn play(&self, sample: &Sample) {
        self.played.lock().unwrap().push(sample.clone());
    }
}

fn down(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        kind: KeyKind::Down,
    }
}

fn up(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        kind: KeyKind::Up,
    }
}

fn player_with_fake() -> (Player, Arc<RecordingSink>, Sample) {
    let sink = Arc::new(RecordingSink::default());
    let click = builtin_click();
    let player = Player::new(sink.clone(), click.clone());
    (player, sink, click)
}

// ---- Built-in click (AC6) ----

fn peak(sample: &Sample) -> f32 {
    sample.samples().iter().fold(0.0, |m, s| m.max(s.abs()))
}

#[test]
fn ac6_click_is_in_memory_and_not_silent() {
    let click = builtin_click();
    assert!(!click.samples().is_empty());
    assert!(peak(&click) > 0.01, "click is silent");
}

#[test]
fn ac6_click_is_shorter_than_250_ms() {
    let click = builtin_click();
    assert!(click.duration() < Duration::from_millis(250));
    assert!(click.duration() > Duration::ZERO);
}

#[test]
fn ac6_click_starts_within_2_ms_of_its_first_sample() {
    let click = builtin_click();
    let threshold = peak(&click) * 0.01;
    let channels = usize::from(click.channels());
    let first = click
        .samples()
        .iter()
        .position(|s| s.abs() > threshold)
        .expect("click has no audible sample");
    let frame = first / channels;
    let seconds = frame as f64 / f64::from(click.sample_rate());
    assert!(seconds <= 0.002, "first audible sample at {seconds} s");
}

#[test]
fn ac6_click_ends_at_zero() {
    let click = builtin_click();
    assert_eq!(*click.samples().last().unwrap(), 0.0);
}

#[test]
fn ac6_click_leaves_headroom_for_four_at_once() {
    assert!(peak(&builtin_click()) <= 0.25);
}

#[test]
fn ac6_click_is_identical_on_every_call() {
    assert_eq!(builtin_click().samples(), builtin_click().samples());
}

#[test]
fn sample_clone_shares_the_buffer_and_new_buffers_differ() {
    let a = builtin_click();
    assert!(a.same_buffer(&a.clone()));
    assert!(!a.same_buffer(&builtin_click()));
}

#[test]
fn sample_reports_its_format_and_duration() {
    let sample = Sample::new(2, 1000, vec![0.0; 2000]);
    assert_eq!(sample.channels(), 2);
    assert_eq!(sample.sample_rate(), 1000);
    assert_eq!(sample.samples().len(), 2000);
    assert_eq!(sample.duration(), Duration::from_secs(1));
}

// ---- Player (AC1 to AC4, AC7) ----

#[test]
fn ac1_one_down_submits_exactly_one_click() {
    let (player, sink, _) = player_with_fake();
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink.count(), 1);
}

#[test]
fn ac2_up_submits_nothing() {
    let (player, sink, _) = player_with_fake();
    player.handle(up(KeyCode::KeyA));
    assert_eq!(sink.count(), 0);
}

#[test]
fn ac2_down_then_up_submits_one() {
    let (player, sink, _) = player_with_fake();
    player.handle(down(KeyCode::KeyA));
    player.handle(up(KeyCode::KeyA));
    assert_eq!(sink.count(), 1);
}

#[test]
fn ac3_every_kind_of_key_submits_one_identical_click() {
    let (player, sink, _) = player_with_fake();
    let codes = [
        KeyCode::KeyA,
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::ShiftLeft,
        KeyCode::F5,
        KeyCode::Unknown(999),
    ];
    for (i, code) in codes.iter().enumerate() {
        player.handle(down(*code));
        assert_eq!(sink.count(), i + 1, "no click for {code}");
    }
    let played = sink.played.lock().unwrap();
    for sample in played.iter() {
        assert!(sample.same_buffer(&played[0]));
    }
}

#[test]
fn ac4_ten_downs_with_no_gap_give_ten_clicks() {
    let (player, sink, _) = player_with_fake();
    for _ in 0..10 {
        player.handle(down(KeyCode::KeyA));
    }
    assert_eq!(sink.count(), 10);
}

#[test]
fn ac7_handling_a_down_is_fast_and_reuses_the_prepared_click() {
    let (player, sink, click) = player_with_fake();
    let mut times = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        player.handle(down(KeyCode::KeyA));
        times.push(start.elapsed());
    }
    times.sort();
    assert!(
        times[500] < Duration::from_millis(1),
        "median {:?}",
        times[500]
    );
    let played = sink.played.lock().unwrap();
    assert_eq!(played.len(), 1000);
    assert!(played.iter().all(|s| s.same_buffer(&click)));
}

// ---- SinkSlot ----

#[test]
fn slot_with_nothing_set_plays_nothing_and_does_not_panic() {
    let slot = SinkSlot::new();
    slot.play(&builtin_click());
}

#[test]
fn slot_forwards_to_the_set_sink_until_cleared() {
    let slot = SinkSlot::new();
    let sink = Arc::new(RecordingSink::default());
    slot.set(sink.clone());
    slot.play(&builtin_click());
    assert_eq!(sink.count(), 1);
    slot.clear();
    slot.play(&builtin_click());
    assert_eq!(sink.count(), 1);
}

#[test]
fn slot_set_replaces_the_previous_sink() {
    let slot = SinkSlot::new();
    let old = Arc::new(RecordingSink::default());
    let new = Arc::new(RecordingSink::default());
    slot.set(old.clone());
    slot.set(new.clone());
    slot.play(&builtin_click());
    assert_eq!((old.count(), new.count()), (0, 1));
}

/// A sink whose first `play` blocks until released.
struct BlockingSink {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    first: AtomicBool,
}

impl Sink for BlockingSink {
    fn play(&self, _sample: &Sample) {
        if self.first.swap(false, Ordering::SeqCst) {
            let _ = self.entered.send(());
            let _ = self.release.lock().unwrap().recv();
        }
    }
}

#[test]
fn slot_play_returns_at_once_while_another_thread_is_inside_it() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let slot = Arc::new(SinkSlot::new());
    slot.set(Arc::new(BlockingSink {
        entered: entered_tx,
        release: Mutex::new(release_rx),
        first: AtomicBool::new(true),
    }));

    let blocked = {
        let slot = slot.clone();
        thread::spawn(move || slot.play(&builtin_click()))
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    let (done_tx, done_rx) = mpsc::channel();
    let second = {
        let slot = slot.clone();
        thread::spawn(move || {
            slot.play(&builtin_click());
            let _ = done_tx.send(());
        })
    };
    let returned = done_rx.recv_timeout(Duration::from_secs(2)).is_ok();

    release_tx.send(()).unwrap();
    blocked.join().unwrap();
    second.join().unwrap();
    assert!(returned, "a second play waited on the first");
}

// ---- Supervisor with a fake back end ----

/// What the fake back end will do, and what it has done.
#[derive(Default)]
struct World {
    /// Results for the next opens; when empty, `open` succeeds.
    open_results: Vec<Result<(), AudioError>>,
    opens: usize,
    sinks: Vec<Arc<RecordingSink>>,
    stale_flags: Vec<Arc<AtomicBool>>,
}

struct FakeBackend {
    world: Arc<Mutex<World>>,
}

/// The fake stream: stale when its flag is set.
struct FakeStream {
    stale: Arc<AtomicBool>,
}

impl Backend for FakeBackend {
    type Stream = FakeStream;

    fn open(&mut self) -> Result<(FakeStream, Arc<dyn Sink>), AudioError> {
        let mut world = self.world.lock().unwrap();
        world.opens += 1;
        if !world.open_results.is_empty() {
            world.open_results.remove(0)?;
        }
        let sink = Arc::new(RecordingSink::default());
        let stale = Arc::new(AtomicBool::new(false));
        world.sinks.push(sink.clone());
        world.stale_flags.push(stale.clone());
        Ok((FakeStream { stale }, sink))
    }

    fn is_stale(&mut self, stream: &FakeStream) -> bool {
        stream.stale.load(Ordering::SeqCst)
    }
}

fn supervisor(
    open_results: Vec<Result<(), AudioError>>,
) -> (Supervisor<FakeBackend>, Arc<SinkSlot>, Arc<Mutex<World>>) {
    let world = Arc::new(Mutex::new(World {
        open_results,
        ..World::default()
    }));
    let slot = Arc::new(SinkSlot::new());
    let supervisor = Supervisor::new(
        FakeBackend {
            world: world.clone(),
        },
        slot.clone(),
    );
    (supervisor, slot, world)
}

fn sink_count(world: &Arc<Mutex<World>>, index: usize) -> usize {
    world.lock().unwrap().sinks[index].count()
}

#[test]
fn ac9_no_device_reports_no_device_and_plays_nothing() {
    let (mut supervisor, slot, world) = supervisor(vec![Err(AudioError::NoDevice)]);
    assert_eq!(supervisor.tick(), OutputStatus::NoDevice);

    let player = Player::new(slot.clone(), builtin_click());
    player.handle(down(KeyCode::KeyA)); // handled without error, plays nothing
    assert!(world.lock().unwrap().sinks.is_empty());
}

#[test]
fn ac9_a_device_that_appears_later_plays_without_restart() {
    let (mut supervisor, slot, world) = supervisor(vec![Err(AudioError::NoDevice)]);
    let player = Player::new(slot.clone(), builtin_click());

    assert_eq!(supervisor.tick(), OutputStatus::NoDevice);
    player.handle(down(KeyCode::KeyA));

    assert_eq!(supervisor.tick(), OutputStatus::Playing);
    assert_eq!(
        sink_count(&world, 0),
        0,
        "a click played in silence was queued"
    );
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink_count(&world, 0), 1);
}

#[test]
fn supervisor_retries_after_several_failed_opens() {
    let (mut supervisor, slot, world) = supervisor(vec![
        Err(AudioError::NoDevice),
        Err(AudioError::Open("busy".into())),
        Err(AudioError::NoDevice),
    ]);
    for _ in 0..3 {
        assert_ne!(supervisor.tick(), OutputStatus::Playing);
        slot.play(&builtin_click());
    }
    assert_eq!(supervisor.tick(), OutputStatus::Playing);
    assert_eq!(world.lock().unwrap().opens, 4);
    slot.play(&builtin_click());
    assert_eq!(sink_count(&world, 0), 1);
}

#[test]
fn supervisor_does_not_reopen_a_healthy_stream() {
    let (mut supervisor, _slot, world) = supervisor(vec![]);
    for _ in 0..5 {
        assert_eq!(supervisor.tick(), OutputStatus::Playing);
    }
    assert_eq!(world.lock().unwrap().opens, 1);
}

#[test]
fn ac10_a_failed_device_drops_clicks_then_resumes_on_the_new_one() {
    let (mut supervisor, slot, world) = supervisor(vec![]);
    let player = Player::new(slot.clone(), builtin_click());

    assert_eq!(supervisor.tick(), OutputStatus::Playing);
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink_count(&world, 0), 1);

    // The device fails, and no replacement can be opened for a while.
    world.lock().unwrap().stale_flags[0].store(true, Ordering::SeqCst);
    world.lock().unwrap().open_results = vec![Err(AudioError::NoDevice)];
    assert_eq!(supervisor.tick(), OutputStatus::NoDevice);
    for _ in 0..5 {
        player.handle(down(KeyCode::KeyA)); // no panic, nothing queued
    }
    assert_eq!(
        sink_count(&world, 0),
        1,
        "old sink got a click after it failed"
    );
    assert_eq!(world.lock().unwrap().sinks.len(), 1);

    // The new default device opens.
    assert_eq!(supervisor.tick(), OutputStatus::Playing);
    assert_eq!(world.lock().unwrap().sinks.len(), 2);
    assert_eq!(
        sink_count(&world, 1),
        0,
        "clicks from the silent gap arrived late"
    );
    player.handle(down(KeyCode::KeyA));
    assert_eq!(sink_count(&world, 1), 1);
    assert_eq!(sink_count(&world, 0), 1);
}

#[test]
fn ac10_a_stale_stream_is_dropped_and_the_slot_stops_using_it() {
    let (mut supervisor, slot, world) = supervisor(vec![]);
    supervisor.tick();
    let flag = world.lock().unwrap().stale_flags[0].clone();
    flag.store(true, Ordering::SeqCst);
    world.lock().unwrap().open_results = vec![Err(AudioError::NoDevice)];
    supervisor.tick();
    // Left: this test's handle and the world's list. The stream is gone.
    assert_eq!(Arc::strong_count(&flag), 2);
    slot.play(&builtin_click());
    assert_eq!(sink_count(&world, 0), 0);
}

#[test]
fn supervisor_survives_an_open_error_of_either_kind() {
    let (mut supervisor, _slot, _world) = supervisor(vec![
        Err(AudioError::Open("refused".into())),
        Err(AudioError::NoDevice),
    ]);
    assert_ne!(supervisor.tick(), OutputStatus::Playing);
    assert_ne!(supervisor.tick(), OutputStatus::Playing);
}

#[test]
fn audio_error_has_readable_messages() {
    assert!(!AudioError::NoDevice.to_string().is_empty());
    assert!(AudioError::Open("busy".into()).to_string().contains("busy"));
}
