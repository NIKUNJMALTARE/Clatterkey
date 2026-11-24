//! Tests for the permission logic (feature 08, AC1 to AC8): the `Watcher`
//! over a fake `Os` and a fake `Keys`, `initial_state`, `open_settings`, and
//! `LiveKeys` over a fake start function. No macOS call, no key capture and no
//! sleeping: `step` is called directly.
//!
//! Signatures assumed (the plan leaves constructors loose):
//!
//! ```ignore
//! Permission::new(PermissionState) -> Permission     // shared as Arc<Permission>
//! Watcher::new(os: Arc<O>, keys: K, permission: Arc<Permission>, err: ErrorOut)
//! Watcher::step(&mut self) -> Duration
//! LiveKeys::new(start: impl FnMut(Consumer) -> Result<Running, KeyListenError> + Send + 'static,
//!               sink: Arc<dyn Sink>, live: Arc<Live>, err: ErrorOut)
//! Running { stop: Box<dyn FnOnce() + Send>, finished: Box<dyn Fn() -> bool + Send> }
//! pub const CHECK_WHILE_MISSING: Duration; pub const CHECK_WHILE_GRANTED: Duration;
//! ```

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clatterkey_lib::audio::{Live, Sink};
use clatterkey_lib::keys::{KeyCode, KeyListenError};
use clatterkey_lib::packs::load;
use clatterkey_lib::permission::{
    CHECK_WHILE_GRANTED, CHECK_WHILE_MISSING, Keys, Os, Permission, PermissionState, Watcher,
    initial_state, open_settings,
};
use clatterkey_lib::startup::{Consumer, LiveKeys, Running};

mod common;
use common::*;

// Fakes

struct FakeOs {
    required: AtomicBool,
    granted: AtomicBool,
    requests: AtomicUsize,
    panes: AtomicUsize,
    pane_error: Mutex<Option<String>>,
}

impl FakeOs {
    fn new(required: bool, granted: bool) -> Arc<Self> {
        Arc::new(Self {
            required: AtomicBool::new(required),
            granted: AtomicBool::new(granted),
            requests: AtomicUsize::new(0),
            panes: AtomicUsize::new(0),
            pane_error: Mutex::new(None),
        })
    }
    fn grant(&self, yes: bool) {
        self.granted.store(yes, Ordering::SeqCst);
    }
    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
    fn panes(&self) -> usize {
        self.panes.load(Ordering::SeqCst)
    }
}

impl Os for FakeOs {
    fn required(&self) -> bool {
        self.required.load(Ordering::SeqCst)
    }
    fn granted(&self) -> bool {
        self.granted.load(Ordering::SeqCst)
    }
    fn request(&self) {
        self.requests.fetch_add(1, Ordering::SeqCst);
    }
    fn open_pane(&self) -> Result<(), String> {
        self.panes.fetch_add(1, Ordering::SeqCst);
        match self.pane_error.lock().unwrap().clone() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
}

/// What the fake listener did, shared with the test.
#[derive(Default)]
struct KeyLog {
    starts: AtomicUsize,
    stops: AtomicUsize,
    fail: AtomicBool,
    live: AtomicUsize,
    max_live: AtomicUsize,
    ended: AtomicBool,
}

struct FakeKeys {
    log: Arc<KeyLog>,
    running: bool,
}

fn fake_keys() -> (FakeKeys, Arc<KeyLog>) {
    let log = Arc::new(KeyLog::default());
    (
        FakeKeys {
            log: Arc::clone(&log),
            running: false,
        },
        log,
    )
}

impl Keys for FakeKeys {
    fn start(&mut self) -> Result<(), KeyListenError> {
        self.log.starts.fetch_add(1, Ordering::SeqCst);
        if self.log.fail.load(Ordering::SeqCst) {
            return Err(KeyListenError::Start("denied".into()));
        }
        self.log.ended.store(false, Ordering::SeqCst);
        let now = self.log.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.log.max_live.fetch_max(now, Ordering::SeqCst);
        self.running = true;
        Ok(())
    }
    fn stop(&mut self) {
        self.log.stops.fetch_add(1, Ordering::SeqCst);
        if self.running {
            self.log.live.fetch_sub(1, Ordering::SeqCst);
        }
        self.running = false;
    }
    fn running(&self) -> bool {
        self.running && !self.log.ended.load(Ordering::SeqCst)
    }
}

struct Rig {
    watcher: Watcher<FakeOs, FakeKeys>,
    os: Arc<FakeOs>,
    log: Arc<KeyLog>,
    permission: Arc<Permission>,
    err: Captured,
}

fn rig(granted: bool) -> Rig {
    let os = FakeOs::new(true, granted);
    let (keys, log) = fake_keys();
    let permission = Arc::new(Permission::new(initial_state(&*os)));
    let (err, out) = fake_err();
    let watcher = Watcher::new(Arc::clone(&os), keys, Arc::clone(&permission), out);
    Rig {
        watcher,
        os,
        log,
        permission,
        err,
    }
}

// AC1, AC2: start-up

#[test]
fn ac1_permission_reported_at_start_starts_the_listener_once_and_is_granted() {
    let mut r = rig(true);
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1);
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(r.os.requests(), 0, "macOS is not asked");
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1, "still once");
    assert_eq!(r.os.requests(), 0);
    assert_eq!(written(&r.err), "");
}

#[test]
fn ac2_permission_missing_at_start_starts_nothing_and_is_needed() {
    let mut r = rig(false);
    assert_eq!(r.permission.state(), PermissionState::Needed);
    for _ in 0..3 {
        r.watcher.step();
    }
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 0);
    assert_eq!(r.permission.state(), PermissionState::Needed);
    assert_eq!(r.os.requests(), 0, "macOS is not asked");
    assert_eq!(r.os.panes(), 0);
    assert_eq!(written(&r.err), "");
}

#[test]
fn ac2_initial_state_is_needed_on_macos_and_never_asks() {
    let os = FakeOs::new(true, false);
    assert_eq!(initial_state(&*os), PermissionState::Needed);
    let os = FakeOs::new(true, true);
    assert_eq!(initial_state(&*os), PermissionState::Needed);
    assert_eq!(os.requests(), 0);
}

// AC3: the grant arrives while running

#[test]
fn ac3_a_grant_starts_the_listener_within_one_step() {
    let mut r = rig(false);
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Needed);
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1);
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(r.os.requests(), 0);
}

struct Live3 {
    watcher: Watcher<FakeOs, LiveKeys<StartFn>>,
    os: Arc<FakeOs>,
    consumers: Arc<Mutex<Vec<Consumer>>>,
    starts: Arc<AtomicUsize>,
    live: Arc<Live>,
    sink: Arc<RecordingSink>,
    permission: Arc<Permission>,
}

type StartFn = Box<dyn FnMut(Consumer) -> Result<Running, KeyListenError> + Send>;

fn live_rig(enabled: bool) -> Live3 {
    let os = FakeOs::new(true, false);
    let consumers: Arc<Mutex<Vec<Consumer>>> = Arc::default();
    let starts = Arc::new(AtomicUsize::new(0));
    let (c, s) = (Arc::clone(&consumers), Arc::clone(&starts));
    let start: StartFn = Box::new(move |consumer| {
        c.lock().unwrap().push(consumer);
        s.fetch_add(1, Ordering::SeqCst);
        Ok(Running {
            stop: Box::new(|| ()),
            finished: Box::new(|| false),
        })
    });
    let live = Arc::new(Live::new(load(&fixture_pack("full")).unwrap().sounds));
    live.set_enabled(enabled);
    let sink = Arc::new(RecordingSink::default());
    let (_, err) = fake_err();
    let keys = LiveKeys::new(
        start,
        sink.clone() as Arc<dyn Sink>,
        Arc::clone(&live),
        err.clone(),
    );
    let permission = Arc::new(Permission::new(initial_state(&*os)));
    let watcher = Watcher::new(Arc::clone(&os), keys, Arc::clone(&permission), err);
    Live3 {
        watcher,
        os,
        consumers,
        starts,
        live,
        sink,
        permission,
    }
}

#[test]
fn ac3_after_the_grant_a_key_event_plays_the_selected_pack_at_the_volume_in_force() {
    let mut r = live_rig(true);
    r.live.set_volume(0.5);
    r.watcher.step();
    assert_eq!(r.starts.load(Ordering::SeqCst), 0);
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.starts.load(Ordering::SeqCst), 1);
    assert_eq!(r.permission.state(), PermissionState::Granted);

    let mut consumers = r.consumers.lock().unwrap();
    consumers[0](down(KeyCode::Space));
    let played = r.sink.samples();
    assert_eq!(played.len(), 1);
    assert_eq!(ms(&played[0]), 70, "the full pack's space sample");
    assert!((played[0].gain() - r.live.gain()).abs() < 1e-6);
    assert!(played[0].gain() < 1.0);
}

#[test]
fn ac3_with_enabled_false_a_key_event_plays_nothing() {
    let mut r = live_rig(false);
    r.watcher.step();
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Granted);
    let mut consumers = r.consumers.lock().unwrap();
    consumers[0](down(KeyCode::KeyA));
    consumers[0](down(KeyCode::Space));
    assert_eq!(r.sink.count(), 0);
}

#[test]
fn a_second_start_plays_with_the_volume_in_force_at_that_start() {
    let mut r = live_rig(true);
    r.os.grant(true);
    r.watcher.step();
    r.live.set_volume(1.0);
    r.consumers.lock().unwrap()[0](down(KeyCode::KeyA));
    // Permission withdrawn and given back, with a volume change between.
    r.os.grant(false);
    r.watcher.step();
    r.live.set_volume(0.25);
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.starts.load(Ordering::SeqCst), 2);
    r.consumers.lock().unwrap()[1](down(KeyCode::KeyA));
    let played = r.sink.samples();
    assert_eq!(played.len(), 2);
    assert!((played[0].gain() - 1.0).abs() < 1e-6);
    assert!((played[1].gain() - r.live.gain()).abs() < 1e-6);
    assert!(played[1].gain() < 0.9);
}

// AC4: the interval

#[test]
fn ac4_the_check_repeats_each_second_while_needed_or_restart_needed() {
    assert_eq!(CHECK_WHILE_MISSING, Duration::from_secs(1));
    let mut r = rig(false);
    assert_eq!(r.watcher.step(), CHECK_WHILE_MISSING);
    r.os.grant(true);
    r.log.fail.store(true, Ordering::SeqCst);
    assert_eq!(r.permission.state(), PermissionState::Needed);
    assert_eq!(r.watcher.step(), CHECK_WHILE_MISSING);
    assert_eq!(r.permission.state(), PermissionState::RestartNeeded);
}

#[test]
fn ac4_once_the_listener_runs_the_check_is_slower() {
    let mut r = rig(true);
    let wait = r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(wait, CHECK_WHILE_GRANTED);
    assert!(CHECK_WHILE_GRANTED > CHECK_WHILE_MISSING);
}

// AC5: the listener cannot start

#[test]
fn ac5_a_failed_start_is_restart_needed_with_one_line_on_err_over_many_steps() {
    let mut r = rig(true);
    r.log.fail.store(true, Ordering::SeqCst);
    for _ in 0..4 {
        r.watcher.step();
    }
    assert_eq!(r.permission.state(), PermissionState::RestartNeeded);
    let text = written(&r.err);
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert!(text.starts_with("key listener: "), "got: {text:?}");
    assert!(text.contains("denied"));
    assert!(
        r.log.starts.load(Ordering::SeqCst) >= 2,
        "the start is tried again on later checks"
    );
}

#[test]
fn ac5_a_later_successful_try_is_granted_with_one_listener() {
    let mut r = rig(true);
    r.log.fail.store(true, Ordering::SeqCst);
    r.watcher.step();
    r.watcher.step();
    r.log.fail.store(false, Ordering::SeqCst);
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(r.log.live.load(Ordering::SeqCst), 1);
    r.watcher.step();
    r.watcher.step();
    assert_eq!(r.log.live.load(Ordering::SeqCst), 1);
    assert_eq!(r.log.max_live.load(Ordering::SeqCst), 1, "never two");
}

// AC6: open_settings

#[test]
fn ac6_open_settings_asks_once_and_opens_the_pane_once_per_call() {
    let os = FakeOs::new(true, false);
    let permission = Permission::new(PermissionState::Needed);
    assert!(!permission.asked());
    assert_eq!(open_settings(&*os, &permission), Ok(true));
    assert_eq!((os.requests(), os.panes()), (1, 1));
    assert!(permission.asked());
    assert_eq!(open_settings(&*os, &permission), Ok(true));
    assert_eq!((os.requests(), os.panes()), (2, 2));
}

#[test]
fn ac6_a_pane_that_cannot_open_fails_with_a_reason_and_leaves_the_state() {
    let os = FakeOs::new(true, false);
    *os.pane_error.lock().unwrap() = Some("no such pane".into());
    for state in [PermissionState::Needed, PermissionState::RestartNeeded] {
        let permission = Permission::new(state);
        let result = open_settings(&*os, &permission);
        assert_eq!(result, Err("no such pane".to_string()));
        assert_eq!(permission.state(), state);
    }
}

// AC7: the permission is removed and given back

#[test]
fn ac7_withdrawn_permission_stops_the_listener_and_is_needed() {
    let mut r = rig(true);
    r.watcher.step();
    r.os.grant(false);
    r.watcher.step();
    assert_eq!(r.log.stops.load(Ordering::SeqCst), 1);
    assert_eq!(r.permission.state(), PermissionState::Needed);
    r.watcher.step();
    assert_eq!(r.log.stops.load(Ordering::SeqCst), 1, "stopped once");
}

#[test]
fn ac7_given_back_it_starts_again_and_never_two_listeners_run() {
    let mut r = rig(true);
    r.watcher.step();
    r.os.grant(false);
    r.watcher.step();
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 2);
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(r.log.live.load(Ordering::SeqCst), 1);
    assert_eq!(r.log.max_live.load(Ordering::SeqCst), 1);
}

#[test]
fn ac7_given_back_but_the_start_fails_is_restart_needed() {
    let mut r = rig(true);
    r.watcher.step();
    r.os.grant(false);
    r.watcher.step();
    r.os.grant(true);
    r.log.fail.store(true, Ordering::SeqCst);
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::RestartNeeded);
}

// AC8: not macOS

#[test]
fn ac8_where_no_permission_is_required_the_state_is_not_needed() {
    let os = FakeOs::new(false, true);
    assert_eq!(initial_state(&*os), PermissionState::NotNeeded);
    assert_eq!(
        Permission::new(initial_state(&*os)).state(),
        PermissionState::NotNeeded
    );
}

#[test]
fn ac8_open_settings_does_nothing_and_says_so() {
    let os = FakeOs::new(false, true);
    let permission = Permission::new(PermissionState::NotNeeded);
    assert_eq!(open_settings(&*os, &permission), Ok(false));
    assert_eq!((os.requests(), os.panes()), (0, 0));
    assert!(!permission.asked());
    assert_eq!(permission.state(), PermissionState::NotNeeded);
}

// The state itself

#[test]
fn the_state_names_are_the_ones_the_window_reads() {
    assert_eq!(PermissionState::NotNeeded.as_str(), "not_needed");
    assert_eq!(PermissionState::Needed.as_str(), "needed");
    assert_eq!(PermissionState::Granted.as_str(), "granted");
    assert_eq!(PermissionState::RestartNeeded.as_str(), "restart_needed");
}

#[test]
fn the_permission_holds_what_was_set_and_remembers_the_button() {
    let p = Permission::new(PermissionState::Needed);
    assert_eq!(p.state(), PermissionState::Needed);
    p.set(PermissionState::RestartNeeded);
    assert_eq!(p.state(), PermissionState::RestartNeeded);
    assert!(!p.asked());
    p.mark_asked();
    assert!(p.asked());
}

// Edge cases

#[test]
fn a_listener_that_ends_by_itself_while_granted_is_one_line_then_a_fresh_start() {
    let mut r = rig(true);
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1);
    r.log.ended.store(true, Ordering::SeqCst);
    r.watcher.step();
    r.watcher.step();
    let text = written(&r.err);
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 2, "a fresh start");
    assert_eq!(r.permission.state(), PermissionState::Granted);
}

#[test]
fn the_quiet_probe_starts_the_listener_when_the_button_was_pressed() {
    let mut r = rig(false);
    r.permission.mark_asked();
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1);
    assert_eq!(r.permission.state(), PermissionState::Granted);
}

#[test]
fn without_the_button_the_probe_is_not_tried() {
    let mut r = rig(false);
    for _ in 0..3 {
        r.watcher.step();
    }
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 0);
}

#[test]
fn a_failed_quiet_probe_writes_nothing_and_stays_needed() {
    let mut r = rig(false);
    r.permission.mark_asked();
    r.log.fail.store(true, Ordering::SeqCst);
    r.watcher.step();
    r.watcher.step();
    assert!(r.log.starts.load(Ordering::SeqCst) >= 1);
    assert_eq!(r.permission.state(), PermissionState::Needed);
    assert_eq!(written(&r.err), "");
}

// The quiet probe: `heard` is the proof, not the start

struct HearingKeys {
    inner: FakeKeys,
    heard: Arc<AtomicBool>,
}

impl Keys for HearingKeys {
    fn start(&mut self) -> Result<(), KeyListenError> {
        self.inner.start()
    }
    fn stop(&mut self) {
        self.inner.stop()
    }
    fn running(&self) -> bool {
        self.inner.running()
    }
    fn heard(&self) -> bool {
        self.heard.load(Ordering::SeqCst)
    }
}

struct ProbeRig {
    watcher: Watcher<FakeOs, HearingKeys>,
    os: Arc<FakeOs>,
    log: Arc<KeyLog>,
    permission: Arc<Permission>,
    heard: Arc<AtomicBool>,
    err: Captured,
}

fn probe_rig() -> ProbeRig {
    let os = FakeOs::new(true, false);
    let (inner, log) = fake_keys();
    let heard = Arc::new(AtomicBool::new(false));
    let permission = Arc::new(Permission::new(initial_state(&*os)));
    let (err, out) = fake_err();
    let watcher = Watcher::new(
        Arc::clone(&os),
        HearingKeys {
            inner,
            heard: Arc::clone(&heard),
        },
        Arc::clone(&permission),
        out,
    );
    ProbeRig {
        watcher,
        os,
        log,
        permission,
        heard,
        err,
    }
}

#[test]
fn probe_started_but_not_heard_stays_needed_with_one_start_and_no_stop() {
    let mut r = probe_rig();
    r.permission.mark_asked();
    for _ in 0..4 {
        r.watcher.step();
        assert_eq!(r.permission.state(), PermissionState::Needed);
    }
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1);
    assert_eq!(r.log.stops.load(Ordering::SeqCst), 0);
    assert_eq!(written(&r.err), "");
}

#[test]
fn probe_before_the_button_starts_nothing() {
    let mut r = probe_rig();
    r.watcher.step();
    r.watcher.step();
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 0);
    assert_eq!(r.permission.state(), PermissionState::Needed);
}

#[test]
fn probe_becomes_granted_once_a_key_is_heard_without_a_second_start() {
    let mut r = probe_rig();
    r.permission.mark_asked();
    r.watcher.step();
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Needed);
    r.heard.store(true, Ordering::SeqCst);
    let wait = r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(wait, CHECK_WHILE_GRANTED);
    assert_eq!(r.log.starts.load(Ordering::SeqCst), 1);
    assert_eq!(r.log.stops.load(Ordering::SeqCst), 0);
}

#[test]
fn probe_os_grant_with_nothing_heard_is_granted() {
    let mut r = probe_rig();
    r.permission.mark_asked();
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Needed);
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert_eq!(
        r.log.starts.load(Ordering::SeqCst),
        1,
        "the probed listener is kept"
    );
    assert_eq!(r.log.stops.load(Ordering::SeqCst), 0);
}

#[test]
fn probe_os_grant_with_nothing_heard_and_no_probe_is_granted() {
    let mut r = probe_rig();
    r.os.grant(true);
    r.watcher.step();
    assert_eq!(r.permission.state(), PermissionState::Granted);
    assert!(!r.heard.load(Ordering::SeqCst));
}

#[test]
fn live_keys_heard_follows_the_consumer_and_each_listener_has_its_own_flag() {
    let r = live_rig(true);
    let mut keys = {
        // A bare LiveKeys over the same kind of start function.
        let consumers = Arc::clone(&r.consumers);
        let start: StartFn = Box::new(move |consumer| {
            consumers.lock().unwrap().push(consumer);
            Ok(Running {
                stop: Box::new(|| ()),
                finished: Box::new(|| false),
            })
        });
        let (_, err) = fake_err();
        LiveKeys::new(
            start,
            r.sink.clone() as Arc<dyn Sink>,
            Arc::clone(&r.live),
            err,
        )
    };
    r.consumers.lock().unwrap().clear();
    assert!(!keys.heard(), "nothing started");
    keys.start().unwrap();
    assert!(!keys.heard(), "started, no key yet");
    r.consumers.lock().unwrap()[0](down(KeyCode::KeyA));
    assert!(keys.heard());
    keys.stop();
    keys.start().unwrap();
    assert!(!keys.heard(), "a fresh listener has its own flag");
    r.consumers.lock().unwrap()[1](down(KeyCode::KeyA));
    assert!(keys.heard());
}
