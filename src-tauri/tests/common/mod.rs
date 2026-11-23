//! Helpers shared by the feature 04 tests: fixture paths, per-test temporary
//! pack folders (no extra crate), WAV bytes made in memory, a recording sink
//! and a fake error output.

#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mechanical_key_sounds_lib::audio::{Sample, Sink};
use mechanical_key_sounds_lib::keys::{KeyCode, KeyEvent, KeyKind};

/// `src-tauri/tests/fixtures`, made by `scripts/make-test-fixtures.sh`.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

pub fn fixture_pack(name: &str) -> PathBuf {
    fixtures().join("packs").join(name)
}

pub fn fixture_audio(name: &str) -> PathBuf {
    fixtures().join("audio").join(name)
}

/// A temporary folder removed on drop, unique per call.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("mks-test-{}-{}-{}", std::process::id(), label, n));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A pack folder built in a temporary directory. The folder is named `id`.
pub struct TestPack {
    pub tmp: TempDir,
    pub dir: PathBuf,
}

impl TestPack {
    /// A folder `id` holding a `pack.json` with exactly `json`.
    pub fn new(id: &str, json: &str) -> Self {
        let tmp = TempDir::new(id);
        let dir = tmp.path().join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("pack.json"), json).unwrap();
        Self { tmp, dir }
    }

    /// Writes `bytes` to `rel` inside the pack, making folders as needed.
    pub fn add(&self, rel: &str, bytes: &[u8]) -> &Self {
        let path = self.dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        self
    }

    /// Copies `tests/fixtures/audio/<fixture>` to `rel` inside the pack.
    pub fn copy_fixture(&self, rel: &str, fixture: &str) -> &Self {
        self.add(rel, &fs::read(fixture_audio(fixture)).unwrap())
    }
}

/// A pack JSON whose `keys.default` lists `files`.
pub fn json_default(files: &[&str]) -> String {
    let list: Vec<String> = files.iter().map(|f| format!("{f:?}")).collect();
    format!(
        r#"{{"name":"T","license":"CC0-1.0","keys":{{"default":[{}]}}}}"#,
        list.join(",")
    )
}

/// A 16-bit mono PCM WAV of a square wave, `ms` long. `ms == 0` makes a WAV
/// with a header and no audio. `amplitude == 0` makes it silent.
pub fn wav_bytes(rate: u32, ms: u32, amplitude: i16) -> Vec<u8> {
    let frames = rate as u64 * ms as u64 / 1000;
    let data_len = (frames * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let v = if i % 2 == 0 { amplitude } else { -amplitude };
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A sink that records every sample it is asked to play.
#[derive(Default)]
pub struct RecordingSink {
    pub played: Mutex<Vec<Sample>>,
}

impl RecordingSink {
    pub fn count(&self) -> usize {
        self.played.lock().unwrap().len()
    }

    pub fn samples(&self) -> Vec<Sample> {
        self.played.lock().unwrap().clone()
    }
}

impl Sink for RecordingSink {
    fn play(&self, sample: &Sample) {
        self.played.lock().unwrap().push(sample.clone());
    }
}

pub type Captured = Arc<Mutex<Vec<u8>>>;
pub type ErrOut = Arc<Mutex<dyn Write + Send>>;

pub fn fake_err() -> (Captured, ErrOut) {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let handle: Arc<Mutex<dyn Write + Send>> = buffer.clone();
    (buffer, handle)
}

pub fn written(buffer: &Captured) -> String {
    String::from_utf8(buffer.lock().unwrap().clone()).unwrap()
}

pub fn down(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        kind: KeyKind::Down,
    }
}

pub fn up(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        kind: KeyKind::Up,
    }
}

/// A sample's length in whole milliseconds, rounded. The fixture packs give
/// each group its own length, so this names the group that played.
pub fn ms(sample: &Sample) -> u64 {
    let d: Duration = sample.duration();
    (d.as_secs_f64() * 1000.0).round() as u64
}

/// The modifier keys in the spec's table.
pub const MODIFIERS: [KeyCode; 10] = [
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::MetaLeft,
    KeyCode::MetaRight,
    KeyCode::CapsLock,
    KeyCode::Fn,
];
