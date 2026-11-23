//! Tests for the dev command: line format and the `run` wrapper.

use super::*;
use std::io::Write;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use mechanical_key_sounds_lib::keys::{KeyCode, KeyEvent, KeyKind, KeyListenError};

type Wait = Box<dyn FnOnce() -> Result<(), KeyListenError>>;
type Consumer = Box<dyn FnMut(KeyEvent) + Send>;
type Buffer = Arc<Mutex<Vec<u8>>>;
type Out = Arc<Mutex<dyn Write + Send>>;

fn code_str(code: ExitCode) -> String {
    format!("{code:?}")
}

fn is_success(code: ExitCode) -> bool {
    code_str(code) == format!("{:?}", ExitCode::SUCCESS)
}

fn buffers() -> (Buffer, Out) {
    let buf: Buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let out: Out = buf.clone();
    (buf, out)
}

fn text(buf: &Buffer) -> String {
    String::from_utf8(buf.lock().unwrap().clone()).unwrap()
}

#[test]
fn ac12_format_line_gives_down_and_up_lines() {
    let down = KeyEvent {
        code: KeyCode::KeyA,
        kind: KeyKind::Down,
    };
    let up = KeyEvent {
        code: KeyCode::KeyA,
        kind: KeyKind::Up,
    };
    assert_eq!(format_line(down), "down KeyA");
    assert_eq!(format_line(up), "up KeyA");
}

#[test]
fn ac12_format_line_names_modifiers_and_unknown_keys() {
    let shift = KeyEvent {
        code: KeyCode::ShiftLeft,
        kind: KeyKind::Up,
    };
    let unknown = KeyEvent {
        code: KeyCode::Unknown(42),
        kind: KeyKind::Down,
    };
    assert_eq!(format_line(shift), "up ShiftLeft");
    assert_eq!(format_line(unknown), "down Unknown(42)");
}

#[test]
fn ac13_start_failure_writes_the_error_and_exits_non_zero() {
    let (_buf, out) = buffers();
    let mut err = Vec::<u8>::new();
    let code = run(
        |_consumer: Consumer| -> Result<Wait, KeyListenError> {
            Err(KeyListenError::Start("tap refused".into()))
        },
        out,
        &mut err,
    );
    assert!(!is_success(code));
    assert!(String::from_utf8(err).unwrap().contains("tap refused"));
}

#[test]
fn ac13_unsupported_platform_error_is_printed() {
    let (_buf, out) = buffers();
    let mut err = Vec::<u8>::new();
    let code = run(
        |_consumer: Consumer| -> Result<Wait, KeyListenError> { Err(KeyListenError::Unsupported) },
        out,
        &mut err,
    );
    assert!(!is_success(code));
    assert!(String::from_utf8(err).unwrap().contains("not supported"));
}

#[test]
fn ac13_listener_stopping_with_an_error_writes_it_and_exits_non_zero() {
    let (_buf, out) = buffers();
    let mut err = Vec::<u8>::new();
    let code = run(
        |_consumer: Consumer| -> Result<Wait, KeyListenError> {
            Ok(Box::new(|| {
                Err(KeyListenError::Stopped("hook died".into()))
            }))
        },
        out,
        &mut err,
    );
    assert!(!is_success(code));
    assert!(String::from_utf8(err).unwrap().contains("hook died"));
}

#[test]
fn ac12_run_prints_a_start_up_line_then_one_line_per_event_in_order() {
    let (buf, out) = buffers();
    let mut err = Vec::<u8>::new();
    let code = run(
        |mut consumer: Consumer| -> Result<Wait, KeyListenError> {
            consumer(KeyEvent {
                code: KeyCode::KeyA,
                kind: KeyKind::Down,
            });
            consumer(KeyEvent {
                code: KeyCode::KeyA,
                kind: KeyKind::Up,
            });
            Ok(Box::new(|| Ok(())))
        },
        out,
        &mut err,
    );
    assert!(is_success(code));
    assert!(err.is_empty());
    let printed = text(&buf);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines.len(), 3, "output was: {printed:?}");
    assert!(
        lines[0].to_lowercase().contains("listening"),
        "first line: {}",
        lines[0]
    );
    assert_eq!(lines[1], "down KeyA");
    assert_eq!(lines[2], "up KeyA");
}

#[test]
fn ac12_start_up_line_is_printed_before_the_listener_is_started() {
    let (buf, out) = buffers();
    let seen = buf.clone();
    let mut err = Vec::<u8>::new();
    let _ = run(
        move |_consumer: Consumer| -> Result<Wait, KeyListenError> {
            assert!(text(&seen).to_lowercase().contains("listening"));
            Ok(Box::new(|| Ok(())))
        },
        out,
        &mut err,
    );
}

#[cfg(target_os = "macos")]
#[test]
fn ac12_macos_start_up_line_mentions_the_permission() {
    let (buf, out) = buffers();
    let mut err = Vec::<u8>::new();
    let _ = run(
        |_consumer: Consumer| -> Result<Wait, KeyListenError> { Ok(Box::new(|| Ok(()))) },
        out,
        &mut err,
    );
    let printed = text(&buf);
    assert!(printed.contains("Accessibility"), "output was: {printed:?}");
    assert!(
        printed.contains("Input Monitoring"),
        "output was: {printed:?}"
    );
}
