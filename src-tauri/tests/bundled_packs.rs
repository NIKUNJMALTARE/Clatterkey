//! Tests for the four committed packs in `src-tauri/resources/packs/`
//! (feature 05: AC1 to AC4). They read the packs straight from the
//! repository, so no bundling or Tauri is involved.
//!
//! Interfaces used (feature 04, unchanged): `packs::load(&Path)`,
//! `audio::decode(Vec<u8>, Duration)`. The file layout inside a pack is not
//! fixed by the spec, so file names are read from each `pack.json`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clickclak_lib::audio::{Sample, decode};
use clickclak_lib::packs;
use serde_json::Value;

const IDS: [&str; 4] = ["blue", "brown", "red", "typewriter"];
const SWITCH_IDS: [&str; 3] = ["blue", "brown", "red"];
const GROUPS: [&str; 5] = ["default", "space", "enter", "backspace", "modifier"];

fn packs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("packs")
}

fn pack_dir(id: &str) -> PathBuf {
    packs_dir().join(id)
}

fn manifest(id: &str) -> Value {
    let text = fs::read_to_string(pack_dir(id).join("pack.json"))
        .unwrap_or_else(|e| panic!("pack {id}: pack.json: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// File names listed under `section` (`keys` or `up`) for `group`.
fn listed(id: &str, section: &str, group: &str) -> Vec<String> {
    manifest(id)
        .get(section)
        .and_then(|s| s.get(group))
        .and_then(|g| g.as_array())
        .map(|a| {
            a.iter()
                .map(|f| f.as_str().expect("file name").to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Every distinct file of a pack, down and up.
fn all_files(id: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for section in ["keys", "up"] {
        for group in GROUPS {
            for f in listed(id, section, group) {
                if seen.insert(f.clone()) {
                    out.push(f);
                }
            }
        }
    }
    out
}

fn read_file(id: &str, name: &str) -> Vec<u8> {
    fs::read(pack_dir(id).join(name)).unwrap_or_else(|e| panic!("pack {id}: {name}: {e}"))
}

fn decoded(id: &str, name: &str) -> Sample {
    decode(read_file(id, name), Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("pack {id}: {name}: {e}"))
}

/// (format tag, channels, rate, bits) from a WAV header.
fn wav_format(bytes: &[u8]) -> (u16, u16, u32, u16) {
    assert!(bytes.len() >= 12, "too short for WAV");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if id == b"fmt " {
            let b = &bytes[pos + 8..];
            return (
                u16::from_le_bytes([b[0], b[1]]),
                u16::from_le_bytes([b[2], b[3]]),
                u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
                u16::from_le_bytes([b[14], b[15]]),
            );
        }
        pos += 8 + len + (len % 2);
    }
    panic!("no fmt chunk");
}

fn peak(s: &Sample) -> f64 {
    s.samples()
        .iter()
        .fold(0.0f64, |m, v| m.max(f64::from(v.abs())))
}

fn rms(s: &Sample) -> f64 {
    let d = s.samples();
    (d.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / d.len() as f64).sqrt()
}

/// In-place radix-2 FFT.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// Magnitude-weighted mean frequency in Hz of the first channel.
fn centroid(s: &Sample) -> f64 {
    let ch = usize::from(s.channels());
    let mono: Vec<f64> = s
        .samples()
        .iter()
        .step_by(ch)
        .map(|v| f64::from(*v))
        .collect();
    let n = mono.len().next_power_of_two().max(2);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    re[..mono.len()].copy_from_slice(&mono);
    fft(&mut re, &mut im);
    let rate = f64::from(s.sample_rate());
    let (mut num, mut den) = (0.0, 0.0);
    for k in 1..n / 2 {
        let mag = (re[k] * re[k] + im[k] * im[k]).sqrt();
        num += mag * (k as f64 * rate / n as f64);
        den += mag;
    }
    num / den
}

fn mean_centroid(id: &str, group: &str) -> f64 {
    let files = listed(id, "keys", group);
    assert!(!files.is_empty(), "pack {id}: no {group} files");
    files.iter().map(|f| centroid(&decoded(id, f))).sum::<f64>() / files.len() as f64
}

fn mean_rms(id: &str, group: &str) -> f64 {
    let files = listed(id, "keys", group);
    files.iter().map(|f| rms(&decoded(id, f))).sum::<f64>() / files.len() as f64
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let md = e.metadata().unwrap();
            if md.is_dir() {
                dir_size(&e.path())
            } else {
                md.len()
            }
        })
        .sum()
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

// AC1

#[test]
fn ac1_each_pack_loads_with_its_name_licence_and_author() {
    let names = [
        ("blue", "Blue switches"),
        ("brown", "Brown switches"),
        ("red", "Red switches"),
        ("typewriter", "Typewriter"),
    ];
    for (id, name) in names {
        let pack = packs::load(&pack_dir(id)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(pack.id, id);
        assert_eq!(pack.name, name, "pack {id}");
        assert_eq!(pack.license, "CC0-1.0", "pack {id}");
        assert_eq!(
            pack.author.as_deref(),
            Some("ClickClak contributors"),
            "pack {id}"
        );
    }
}

#[test]
fn ac1_a_licence_file_sits_beside_the_packs() {
    let text = fs::read_to_string(packs_dir().join("LICENSE.md")).expect("LICENSE.md");
    assert!(text.contains("CC0"), "licence file should name CC0");
}

// AC2

#[test]
fn ac2_every_pack_has_all_down_groups_with_enough_variations() {
    for id in IDS {
        for group in GROUPS {
            let n = listed(id, "keys", group).len();
            let min = match (group, id) {
                ("default", _) => 4,
                ("enter", "typewriter") => 1,
                _ => 2,
            };
            assert!(n >= min, "pack {id} keys.{group}: {n} files, need {min}");
        }
    }
}

#[test]
fn ac2_switch_packs_have_up_default_and_typewriter_has_no_up() {
    for id in SWITCH_IDS {
        assert!(
            !listed(id, "up", "default").is_empty(),
            "pack {id} needs up.default"
        );
    }
    let m = manifest("typewriter");
    let up = m.get("up");
    let empty = match up {
        None | Some(Value::Null) => true,
        Some(v) => v.as_object().is_none_or(|o| {
            o.values()
                .all(|g| g.as_array().is_none_or(|a| a.is_empty()))
        }),
    };
    assert!(empty, "typewriter must have no up sounds");
    for group in GROUPS {
        assert!(listed("typewriter", "up", group).is_empty());
    }
}

// AC3

#[test]
fn ac3_every_sample_is_16_bit_44k_wav_not_silent_and_peaks_below_minus_1_dbfs() {
    // -1 dBFS is 0.8913 of full scale.
    let limit = 10f64.powf(-1.0 / 20.0) + 1e-4;
    for id in IDS {
        let files = all_files(id);
        assert!(!files.is_empty());
        for f in files {
            let bytes = read_file(id, &f);
            let (tag, _ch, rate, bits) = wav_format(&bytes);
            assert_eq!(tag, 1, "pack {id} {f}: PCM");
            assert_eq!(rate, 44_100, "pack {id} {f}: sample rate");
            assert_eq!(bits, 16, "pack {id} {f}: bit depth");
            let s =
                decode(bytes, Duration::from_secs(30)).unwrap_or_else(|e| panic!("{id} {f}: {e}"));
            let p = peak(&s);
            assert!(p > 0.0, "pack {id} {f}: silent");
            assert!(p <= limit, "pack {id} {f}: peak {p} above -1 dBFS");
        }
    }
}

#[test]
fn ac3_default_groups_are_matched_in_loudness_within_3_db() {
    let levels: Vec<(f64, &str)> = IDS
        .iter()
        .map(|id| (mean_rms(id, "default"), *id))
        .collect();
    let max = levels.iter().map(|l| l.0).fold(f64::MIN, f64::max);
    let min = levels.iter().map(|l| l.0).fold(f64::MAX, f64::min);
    assert!(min > 0.0);
    let db = 20.0 * (max / min).log10();
    assert!(db <= 3.0, "default RMS spread is {db:.2} dB: {levels:?}");
}

#[test]
fn ac3_the_packs_total_under_5_mb() {
    let size = dir_size(&packs_dir());
    assert!(size < 5_000_000, "packs are {size} bytes");
}

// AC4

#[test]
fn ac4_default_brightness_is_blue_then_brown_then_red() {
    let (blue, brown, red) = (
        mean_centroid("blue", "default"),
        mean_centroid("brown", "default"),
        mean_centroid("red", "default"),
    );
    assert!(blue > brown, "blue {blue:.0} Hz vs brown {brown:.0} Hz");
    assert!(brown > red, "brown {brown:.0} Hz vs red {red:.0} Hz");
}

#[test]
fn ac4_space_is_lower_than_default_in_every_pack() {
    for id in IDS {
        let space = mean_centroid(id, "space");
        let default = mean_centroid(id, "default");
        assert!(
            space < default,
            "pack {id}: space {space:.0} Hz, default {default:.0} Hz"
        );
    }
}

#[test]
fn ac4_typewriter_enter_is_longer_than_half_a_second() {
    let files = listed("typewriter", "keys", "enter");
    assert!(!files.is_empty());
    for f in files {
        let d = decoded("typewriter", &f).duration();
        assert!(d > Duration::from_millis(500), "{f} is {d:?}");
    }
}

#[test]
fn ac4_no_two_files_across_the_packs_are_identical() {
    let mut files = Vec::new();
    for id in IDS {
        files_under(&pack_dir(id), &mut files);
    }
    let wavs: Vec<&PathBuf> = files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "wav"))
        .collect();
    assert!(wavs.len() >= 40, "only {} wav files", wavs.len());
    let mut seen: HashSet<Vec<u8>> = HashSet::new();
    for p in wavs {
        assert!(
            seen.insert(fs::read(p).unwrap()),
            "duplicate file: {}",
            p.display()
        );
    }
}

// Feature 09 AC18: the bundled packs' key-up flags.

#[test]
fn feature_09_ac18_the_switch_packs_have_key_up_sounds_and_typewriter_does_not() {
    for id in IDS {
        let info = packs::read_info(&pack_dir(id)).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(
            info.has_key_up,
            SWITCH_IDS.contains(&id),
            "pack {id}: has_key_up"
        );
    }
}
