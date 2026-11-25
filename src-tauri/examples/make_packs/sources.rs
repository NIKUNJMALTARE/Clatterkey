//! Recorded source clips: the list in `sources/sources.json`, its checks,
//! and cutting single key presses out of a clip.
//!
//! Only CC0 recordings may be used, because the packs are copied into every
//! installer. The whole list is checked before anything is written, so a
//! bad entry stops the run with one line naming the clip.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::voices::Pack;
use crate::wav;

/// The only licence accepted for a source clip.
pub const LICENSE: &str = "CC0-1.0";

/// The `source` of a recording made by the project's own contributors.
pub const OWN: &str = "own recording";

/// One checked, decoded source clip.
#[derive(Debug)]
pub struct Clip {
    /// File name inside the sources folder.
    pub file: String,
    pub pack: Pack,
    pub title: String,
    pub author: String,
    /// A web page address, or `own recording`.
    pub source: String,
    pub license: String,
    pub samples: Vec<f64>,
}

/// Reads and checks `sources.json` and every clip in `dir`. A folder that
/// does not exist, or has no `sources.json`, gives no clips.
pub fn load(dir: &Path) -> Result<Vec<Clip>, String> {
    let list_path = dir.join("sources.json");
    if !list_path.exists() {
        // A folder of clips with no list is still refused below.
        return check_unlisted(dir, &BTreeSet::new()).map(|()| Vec::new());
    }
    let text = fs::read_to_string(&list_path).map_err(|e| format!("sources.json: {e}"))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("sources.json: {e}"))?;
    let entries = value
        .get("clips")
        .and_then(Value::as_array)
        .ok_or("sources.json: \"clips\" must be a list")?;

    let mut clips = Vec::new();
    let mut listed = BTreeSet::new();
    for (i, entry) in entries.iter().enumerate() {
        let field = |name: &str| -> Result<String, String> {
            entry
                .get(name)
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    let who = entry
                        .get("file")
                        .and_then(Value::as_str)
                        .map_or_else(|| format!("clips[{i}]"), |f| format!("\"{f}\""));
                    format!("source {who}: missing \"{name}\"")
                })
        };
        let file = field("file")?;
        let named = |msg: String| format!("source \"{file}\": {msg}");
        if file.contains('/') || file.contains('\\') || file.starts_with('.') {
            return Err(named(
                "must be a plain file name in the sources folder".into(),
            ));
        }
        let license = field("license")?;
        if license != LICENSE {
            return Err(named(format!("licence \"{license}\" is not {LICENSE}")));
        }
        let pack_id = field("pack")?;
        let pack = Pack::ALL
            .iter()
            .copied()
            .find(|p| p.id() == pack_id)
            .ok_or_else(|| named(format!("unknown pack \"{pack_id}\"")))?;
        let title = field("title")?;
        let author = field("author")?;
        let source = field("source")?;
        if source != OWN && !source.starts_with("https://") {
            return Err(named(format!(
                "source must be a https:// page or \"{OWN}\""
            )));
        }
        if !listed.insert(file.clone()) {
            return Err(named("listed twice".into()));
        }
        let bytes = fs::read(dir.join(&file)).map_err(|e| named(e.to_string()))?;
        let samples = wav::decode(&bytes).map_err(named)?;
        clips.push(Clip {
            file,
            pack,
            title,
            author,
            source,
            license,
            samples,
        });
    }
    check_unlisted(dir, &listed)?;
    Ok(clips)
}

/// Refuses any `.wav` in `dir` that the list does not name.
fn check_unlisted(dir: &Path, listed: &BTreeSet<String>) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".wav"))
        .collect();
    names.sort();
    match names.into_iter().find(|n| !listed.contains(n)) {
        Some(name) => Err(format!("source \"{name}\": not listed in sources.json")),
        None => Ok(()),
    }
}

/// Window of the energy envelope, in samples (about 2 ms).
const WINDOW: usize = 88;
/// Shortest gap between two presses, in samples (60 ms).
const MIN_GAP: usize = 2646;
/// Length of a cut press, in samples (80 ms).
const PRESS: usize = 3528;
/// Lead before the onset, in samples (2 ms).
const LEAD: usize = 88;
/// An onset is where the envelope rises above this multiple of the median.
const RISE: f64 = 30.0;

/// Single key presses found in a clip, in order: each cut from just before
/// its onset, faded in, and faded out over its tail. Presses that reach full
/// scale are left out, as they are clipped.
pub fn presses(clip: &[f64]) -> Vec<Vec<f64>> {
    if clip.len() < WINDOW * 2 {
        return Vec::new();
    }
    // Mean square over non-overlapping windows.
    let env: Vec<f64> = clip
        .chunks(WINDOW)
        .map(|w| w.iter().map(|s| s * s).sum::<f64>() / w.len() as f64)
        .collect();
    let mut sorted = env.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    // A clip of silence, or one so loud there is no quiet floor, has no
    // presses to find.
    let threshold = (median * RISE).max(1e-6);

    let mut out = Vec::new();
    let mut last: Option<usize> = None;
    let mut below = true;
    for (i, &e) in env.iter().enumerate() {
        if e <= threshold {
            below = true;
            continue;
        }
        if !below {
            continue;
        }
        below = false;
        let onset = i * WINDOW;
        if last.is_some_and(|l| onset < l + MIN_GAP) {
            continue;
        }
        last = Some(onset);
        let start = onset.saturating_sub(LEAD);
        if start + PRESS > clip.len() {
            break;
        }
        let mut press = clip[start..start + PRESS].to_vec();
        if press.iter().any(|s| s.abs() >= 0.999) {
            continue;
        }
        shape(&mut press);
        out.push(press);
    }
    out
}

/// Fades a cut press in over its lead and out over its last 30 ms.
fn shape(press: &mut [f64]) {
    for (i, s) in press.iter_mut().take(LEAD).enumerate() {
        *s *= i as f64 / LEAD as f64;
    }
    let tail = 1323.min(press.len());
    let len = press.len();
    for i in 0..tail {
        let k = i as f64 / tail as f64;
        press[len - 1 - i] *= k * k;
    }
}
