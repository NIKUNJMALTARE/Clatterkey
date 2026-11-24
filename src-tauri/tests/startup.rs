//! Tests for the start-up wiring: the key consumer and `run_clicks`, with a
//! fake listener and a fake sink. No real key capture and no sound device,
//! except AC12 which calls the real `keys::start` where it is unsupported.
//!
//! Signatures assumed (the plan leaves them loose; they follow `run` in
//! `examples/keys/main.rs`):
//!
//! ```ignore
//! pub fn key_consumer(player: Player, err: Arc<Mutex<dyn Write + Send>>)
//!     -> impl FnMut(KeyEvent) + Send + 'static;
//! pub fn run_clicks(
//!     start: impl FnOnce(Box<dyn FnMut(KeyEvent) + Send>)
//!         -> Result<Box<dyn FnOnce() -> Result<(), KeyListenError>>, KeyListenError>,
//!     sink: Arc<dyn Sink>,
//!     err: Arc<Mutex<dyn Write + Send>>,
//! );   // returns ()
//! ```

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use clatterkey_lib::audio::{Player, Sample, Sink, builtin_click};
use clatterkey_lib::keys::{KeyCode, KeyEvent, KeyKind, KeyListenError};
use clatterkey_lib::startup::{key_consumer, run_clicks};

type Wait = Box<dyn FnOnce() -> Result<(), KeyListenError>>;

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

/// A sink that counts calls and panics on each.
#[derive(Default)]
struct PanickingSink {
    calls: AtomicUsize,
}

impl Sink for PanickingSink {
    fn play(&self, _sample: &Sample) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        panic!("sink failed");
    }
}

/// Standard error stand-in: the bytes written so far, and the handle to give.
type Captured = Arc<Mutex<Vec<u8>>>;
type ErrOut = Arc<Mutex<dyn Write + Send>>;

fn fake_err() -> (Captured, ErrOut) {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let handle: Arc<Mutex<dyn Write + Send>> = buffer.clone();
    (buffer, handle)
}

fn written(buffer: &Captured) -> String {
    String::from_utf8(buffer.lock().unwrap().clone()).unwrap()
}

fn event(code: KeyCode, kind: KeyKind) -> KeyEvent {
    KeyEvent { code, kind }
}

#[test]
fn ac11_a_listener_that_refuses_to_start_gives_one_error_line_and_no_click() {
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();

    run_clicks(
        |_consumer| Err(KeyListenError::Start("refused".into())),
        sink.clone(),
        err,
    );

    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert!(text.starts_with("key listener: "), "got: {text:?}");
    assert!(text.contains("refused"));
    assert_eq!(sink.count(), 0);
}

#[test]
fn ac11_a_listener_that_stops_with_an_error_gives_one_error_line() {
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();

    run_clicks(
        |_consumer| Ok(Box::new(|| Err(KeyListenError::Stopped("boom".into()))) as Wait),
        sink.clone(),
        err,
    );

    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert!(text.starts_with("key listener: "), "got: {text:?}");
    assert!(text.contains("boom"));
    assert_eq!(sink.count(), 0);
}

#[test]
fn ac11_a_healthy_listener_plays_a_click_on_down_only_and_writes_nothing() {
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();

    run_clicks(
        |mut consumer| {
            Ok(Box::new(move || {
                consumer(event(KeyCode::KeyA, KeyKind::Down));
                consumer(event(KeyCode::KeyA, KeyKind::Up));
                consumer(event(KeyCode::Space, KeyKind::Down));
                Ok(())
            }) as Wait)
        },
        sink.clone(),
        err,
    );

    assert_eq!(written(&buffer), "");
    let played = sink.played.lock().unwrap();
    assert_eq!(played.len(), 2);
    assert_eq!(played[0].samples(), builtin_click().samples());
    assert!(played[0].same_buffer(&played[1]));
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn ac12_on_an_unsupported_platform_run_clicks_reports_it_and_returns() {
    use clatterkey_lib::keys;

    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();

    run_clicks(
        |consumer| {
            let listener = keys::start(consumer)?;
            Ok(Box::new(move || listener.wait()) as Wait)
        },
        sink.clone(),
        err,
    );

    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert!(text.contains("not supported"), "got: {text:?}");
    assert_eq!(sink.count(), 0);
}

#[test]
fn ac13_handling_keys_writes_nothing() {
    let sink = Arc::new(RecordingSink::default());
    let (buffer, err) = fake_err();
    let mut consumer = key_consumer(Player::new(sink.clone(), builtin_click()), err);

    for code in [
        KeyCode::KeyA,
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::F5,
        KeyCode::Unknown(999),
    ] {
        consumer(event(code, KeyKind::Down));
        consumer(event(code, KeyKind::Up));
    }

    assert_eq!(written(&buffer), "");
    assert_eq!(sink.count(), 5);
}

#[test]
fn a_panicking_sink_never_unwinds_and_is_reported_once() {
    let sink = Arc::new(PanickingSink::default());
    let (buffer, err) = fake_err();
    let mut consumer = key_consumer(Player::new(sink.clone(), builtin_click()), err);

    for _ in 0..5 {
        consumer(event(KeyCode::KeyA, KeyKind::Down)); // must not panic
    }

    let text = written(&buffer);
    assert_eq!(text.lines().count(), 1, "got: {text:?}");
    assert_eq!(
        sink.calls.load(Ordering::SeqCst),
        1,
        "sink called after the panic"
    );
}
