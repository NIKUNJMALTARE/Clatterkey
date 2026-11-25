//! Tests for the pack generator (feature 05: AC5, feature 15: AC7 and AC8).
//!
//! Interfaces assumed from the feature 15 plan, in `main.rs` (included there
//! as `#[cfg(test)] mod tests;`):
//!
//! ```ignore
//! fn generate(out: &Path, sources_dir: &Path) -> Result<(), String>
//! wav::decode(&[u8]) -> Result<Vec<f64>, String>
//! sources::load(&Path) -> Result<Vec<sources::Clip>, String>
//! sources::presses(&[f64]) -> Vec<Vec<f64>>
//! ```
//!
//! `generate` writes the eight pack folders and `LICENSE.md` into `out`. It
//! loads and checks the sources before it touches `out`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{generate, sources, wav};

/// A temporary folder removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "mks-makepacks-{}-{}-{}",
            std::process::id(),
            label,
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Temp(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every file under `root`, keyed by its `/`-separated relative path.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

const IDS: [&str; 8] = [
    "blue",
    "brown",
    "red",
    "typewriter",
    "thocky-linear",
    "fast-linear",
    "sharp-clicky",
    "tactile-bump",
];

/// The committed sources folder, beside the generator.
fn committed_sources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("make_packs")
        .join("sources")
}

fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("packs")
}

fn assert_same(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>, what: &str) {
    let only_a: Vec<_> = a.keys().filter(|k| !b.contains_key(*k)).collect();
    let only_b: Vec<_> = b.keys().filter(|k| !a.contains_key(*k)).collect();
    assert!(only_a.is_empty(), "{what}: only in first: {only_a:?}");
    assert!(only_b.is_empty(), "{what}: only in second: {only_b:?}");
    let differing: Vec<_> = a
        .iter()
        .filter(|(k, v)| &b[*k] != *v)
        .map(|(k, _)| k)
        .collect();
    assert!(
        differing.is_empty(),
        "{what}: bytes differ in {differing:?}"
    );
}

#[test]
fn ac7_the_generator_writes_the_eight_packs_and_the_licence() {
    let out = Temp::new("shape");
    generate(&out.0, &committed_sources()).unwrap();
    let files = tree(&out.0);
    assert!(files.contains_key("LICENSE.md"));
    for id in IDS {
        assert!(
            files.contains_key(&format!("{id}/pack.json")),
            "{id}/pack.json"
        );
        assert!(
            files
                .keys()
                .any(|k| k.starts_with(&format!("{id}/")) && k.ends_with(".wav")),
            "{id} has no wav"
        );
    }
}

#[test]
fn ac7_generating_twice_gives_byte_identical_files() {
    let (a, b) = (Temp::new("first"), Temp::new("second"));
    generate(&a.0, &committed_sources()).unwrap();
    generate(&b.0, &committed_sources()).unwrap();
    assert_same(&tree(&a.0), &tree(&b.0), "two runs");
}

#[test]
fn ac7_generating_into_an_empty_folder_matches_the_committed_packs_in_both_directions() {
    let out = Temp::new("committed");
    generate(&out.0, &committed_sources()).unwrap();
    let committed = tree(&committed());
    assert!(!committed.is_empty(), "no committed packs");
    // assert_same checks for files only in either side, and for bytes.
    assert_same(&tree(&out.0), &committed, "generated vs committed");
    assert_same(&committed, &tree(&out.0), "committed vs generated");
}

// WAV reader

/// A WAV file with the given format fields and data bytes, with `extra`
/// chunks (id, body) written before `data`.
fn wav_file(
    tag: u16,
    channels: u16,
    rate: u32,
    bits: u16,
    extra: &[(&[u8; 4], Vec<u8>)],
    data: &[u8],
    fmt_extension: Option<&[u8]>,
) -> Vec<u8> {
    let block = channels * bits / 8;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&tag.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    fmt.extend_from_slice(&block.to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());
    if let Some(ext) = fmt_extension {
        fmt.extend_from_slice(ext);
    }
    let mut body = Vec::new();
    body.extend_from_slice(b"WAVE");
    body.extend_from_slice(b"fmt ");
    body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    body.extend_from_slice(&fmt);
    for (id, content) in extra {
        body.extend_from_slice(*id);
        body.extend_from_slice(&(content.len() as u32).to_le_bytes());
        body.extend_from_slice(content);
        if content.len() % 2 == 1 {
            body.push(0);
        }
    }
    body.extend_from_slice(b"data");
    body.extend_from_slice(&(data.len() as u32).to_le_bytes());
    body.extend_from_slice(data);
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

fn pcm16(values: &[i16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn pcm24(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|v| v.to_le_bytes()[..3].to_vec())
        .collect()
}

/// The WAVE_FORMAT_EXTENSIBLE extension: size 22, valid bits, mask, and the
/// PCM sub-format GUID.
fn extensible_tail(bits: u16) -> Vec<u8> {
    let mut e = Vec::new();
    e.extend_from_slice(&22u16.to_le_bytes());
    e.extend_from_slice(&bits.to_le_bytes());
    e.extend_from_slice(&4u32.to_le_bytes());
    e.extend_from_slice(&[
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B,
        0x71,
    ]);
    e
}

#[test]
fn ac8_the_wav_reader_decodes_16_bit_by_exact_division() {
    let bytes = wav_file(
        1,
        1,
        44_100,
        16,
        &[],
        &pcm16(&[0, 16_384, -16_384, 32_767, -32_768]),
        None,
    );
    let s = wav::decode(&bytes).unwrap();
    assert_eq!(s, [0.0, 0.5, -0.5, 32_767.0 / 32_768.0, -1.0]);
}

#[test]
fn ac8_the_wav_reader_decodes_24_bit_by_exact_division() {
    let bytes = wav_file(
        1,
        1,
        44_100,
        24,
        &[],
        &pcm24(&[0, 4_194_304, -4_194_304, 8_388_607, -8_388_608]),
        None,
    );
    let s = wav::decode(&bytes).unwrap();
    assert_eq!(s, [0.0, 0.5, -0.5, 8_388_607.0 / 8_388_608.0, -1.0]);
}

#[test]
fn ac8_the_wav_reader_reads_the_extensible_pcm_format() {
    let bytes = wav_file(
        0xFFFE,
        1,
        44_100,
        16,
        &[],
        &pcm16(&[16_384, -16_384]),
        Some(&extensible_tail(16)),
    );
    assert_eq!(wav::decode(&bytes).unwrap(), [0.5, -0.5]);
}

#[test]
fn ac8_the_wav_reader_skips_unknown_chunks_before_data() {
    let extra: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"LIST", b"INFOabcde".to_vec()), // odd length: padded
        (b"junk", vec![1, 2, 3, 4]),
    ];
    let bytes = wav_file(1, 1, 44_100, 16, &extra, &pcm16(&[8_192, -8_192, 0]), None);
    assert_eq!(wav::decode(&bytes).unwrap(), [0.25, -0.25, 0.0]);
}

#[test]
fn ac8_the_wav_reader_refuses_stereo() {
    let bytes = wav_file(1, 2, 44_100, 16, &[], &pcm16(&[1, 2, 3, 4]), None);
    assert!(wav::decode(&bytes).is_err());
}

#[test]
fn ac8_the_wav_reader_refuses_48_khz() {
    let bytes = wav_file(1, 1, 48_000, 16, &[], &pcm16(&[1, 2, 3, 4]), None);
    assert!(wav::decode(&bytes).is_err());
}

#[test]
fn ac8_the_wav_reader_refuses_other_depths_and_float() {
    let eight = wav_file(1, 1, 44_100, 8, &[], &[1, 2, 3, 4], None);
    assert!(wav::decode(&eight).is_err(), "8-bit");
    let float = wav_file(3, 1, 44_100, 32, &[], &[0; 8], None);
    assert!(wav::decode(&float).is_err(), "float");
}

#[test]
fn ac8_the_wav_reader_refuses_truncated_data_and_non_wav_bytes() {
    let mut bytes = wav_file(1, 1, 44_100, 16, &[], &pcm16(&[1, 2, 3, 4]), None);
    bytes.truncate(bytes.len() - 3);
    assert!(wav::decode(&bytes).is_err(), "truncated data");
    assert!(wav::decode(b"not a wav file at all").is_err());
    assert!(wav::decode(&[]).is_err());
}

#[test]
fn ac8_the_wav_reader_decodes_what_the_generator_writes() {
    let input = [0.0, 0.25, -0.5, 0.75];
    let out = wav::decode(&wav::encode(&input)).unwrap();
    assert_eq!(out.len(), input.len());
    for (a, b) in out.iter().zip(input) {
        assert!((a - b).abs() < 1.0 / 16_000.0, "{a} vs {b}");
    }
}

// Sources: the list checks (AC8)

const CLIP_ENTRY: &str = r#""pack": "thocky-linear", "title": "Typing", "author": "Someone",
    "source": "https://freesound.org/people/x/sounds/1/""#;

fn write_clip(dir: &Path, name: &str) {
    let samples: Vec<i16> = (0..2_000).map(|i| ((i % 50) as i16 - 25) * 300).collect();
    fs::write(
        dir.join(name),
        wav_file(1, 1, 44_100, 16, &[], &pcm16(&samples), None),
    )
    .unwrap();
}

fn write_list(dir: &Path, entries: &[String]) {
    fs::write(
        dir.join("sources.json"),
        format!("{{ \"clips\": [{}] }}", entries.join(",")),
    )
    .unwrap();
}

fn entry(file: &str, license: &str) -> String {
    format!(r#"{{ "file": "{file}", {CLIP_ENTRY}, "license": "{license}" }}"#)
}

/// An output folder holding only a marker file.
fn marked_out() -> Temp {
    let out = Temp::new("marked");
    fs::write(out.0.join("marker.txt"), "keep").unwrap();
    out
}

fn assert_untouched(out: &Temp) {
    let files = tree(&out.0);
    assert_eq!(
        files.keys().collect::<Vec<_>>(),
        ["marker.txt"],
        "the generator wrote into the output folder"
    );
    assert_eq!(files["marker.txt"], b"keep");
}

fn one_line(error: &str) -> bool {
    !error.trim().is_empty() && !error.trim().contains('\n')
}

#[test]
fn ac8_the_committed_sources_load() {
    sources::load(&committed_sources()).unwrap();
}

#[test]
fn ac8_a_valid_cc0_clip_is_accepted() {
    let dir = Temp::new("src-ok");
    write_clip(&dir.0, "a.wav");
    write_list(&dir.0, &[entry("a.wav", "CC0-1.0")]);
    let clips = sources::load(&dir.0).unwrap();
    assert_eq!(clips.len(), 1);
    let out = Temp::new("out-ok");
    generate(&out.0, &dir.0).unwrap();
    assert!(tree(&out.0).contains_key("thocky-linear/pack.json"));
}

#[test]
fn ac8_a_clip_with_another_licence_is_refused_by_name_and_nothing_is_written() {
    let dir = Temp::new("src-licence");
    write_clip(&dir.0, "noisy-room.wav");
    write_list(&dir.0, &[entry("noisy-room.wav", "CC-BY-4.0")]);
    let out = marked_out();
    let error = generate(&out.0, &dir.0).expect_err("must refuse");
    assert!(error.contains("noisy-room.wav"), "{error}");
    assert!(one_line(&error), "{error:?}");
    assert_untouched(&out);
    assert!(sources::load(&dir.0).is_err());
}

#[test]
fn ac8_a_clip_in_the_folder_that_the_list_does_not_name_is_refused() {
    let dir = Temp::new("src-unlisted");
    write_clip(&dir.0, "listed.wav");
    write_clip(&dir.0, "stray.wav");
    write_list(&dir.0, &[entry("listed.wav", "CC0-1.0")]);
    let out = marked_out();
    let error = generate(&out.0, &dir.0).expect_err("must refuse");
    assert!(error.contains("stray.wav"), "{error}");
    assert!(one_line(&error), "{error:?}");
    assert_untouched(&out);
}

#[test]
fn ac8_a_listed_clip_whose_file_is_missing_is_refused() {
    let dir = Temp::new("src-missing");
    write_list(&dir.0, &[entry("ghost.wav", "CC0-1.0")]);
    let out = marked_out();
    let error = generate(&out.0, &dir.0).expect_err("must refuse");
    assert!(error.contains("ghost.wav"), "{error}");
    assert!(one_line(&error), "{error:?}");
    assert_untouched(&out);
}

#[test]
fn ac8_an_entry_with_a_missing_field_or_an_unknown_pack_or_a_bad_source_is_refused() {
    let cases = [
        // no author
        r#"{ "file": "a.wav", "pack": "thocky-linear", "title": "T",
             "source": "own recording", "license": "CC0-1.0" }"#,
        // unknown pack
        r#"{ "file": "a.wav", "pack": "no-such-pack", "title": "T", "author": "A",
             "source": "own recording", "license": "CC0-1.0" }"#,
        // source is neither a page address nor "own recording"
        r#"{ "file": "a.wav", "pack": "thocky-linear", "title": "T", "author": "A",
             "source": "somewhere", "license": "CC0-1.0" }"#,
    ];
    for (i, case) in cases.iter().enumerate() {
        let dir = Temp::new("src-bad-entry");
        write_clip(&dir.0, "a.wav");
        write_list(&dir.0, &[case.to_string()]);
        let error = sources::load(&dir.0).expect_err(&format!("case {i} must refuse"));
        assert!(one_line(&error), "case {i}: {error:?}");
        let out = marked_out();
        assert!(generate(&out.0, &dir.0).is_err(), "case {i}");
        assert_untouched(&out);
    }
}

#[test]
fn ac8_own_recording_is_an_accepted_source() {
    let dir = Temp::new("src-own");
    write_clip(&dir.0, "mine.wav");
    write_list(
        &dir.0,
        &[
            r#"{ "file": "mine.wav", "pack": "fast-linear", "title": "My keys",
             "author": "Me", "source": "own recording", "license": "CC0-1.0" }"#
                .to_string(),
        ],
    );
    assert_eq!(sources::load(&dir.0).unwrap().len(), 1);
}

#[test]
fn ac8_a_clip_that_is_not_44_1_khz_pcm_is_refused_by_name() {
    let dir = Temp::new("src-48k");
    fs::write(
        dir.0.join("a.wav"),
        wav_file(1, 1, 48_000, 16, &[], &pcm16(&[1, 2, 3]), None),
    )
    .unwrap();
    write_list(&dir.0, &[entry("a.wav", "CC0-1.0")]);
    let error = sources::load(&dir.0).expect_err("must refuse");
    assert!(error.contains("a.wav"), "{error}");
}

// Recorded layers: onset detection

#[test]
fn the_press_detector_finds_nothing_in_silence() {
    let silence = vec![0.0; 44_100];
    assert!(sources::presses(&silence).is_empty());
}

/// A click: a short decaying burst of a fixed tone, starting at `at`.
fn click(buf: &mut [f64], at: usize) {
    for i in 0..1_500 {
        let t = i as f64;
        let v = (t * 0.9).sin() * (-t / 200.0).exp() * 0.5;
        buf[at + i] += v;
    }
}

/// Faint steady noise (a fixed pattern), so the median is not zero.
fn floor() -> Vec<f64> {
    (0..44_100 * 2)
        .map(|i| (((i * 7919) % 201) as f64 / 100.0 - 1.0) * 0.001)
        .collect()
}

#[test]
fn the_press_detector_finds_one_press_in_a_clip_with_one_press() {
    let mut clip = floor();
    click(&mut clip, 30_000);
    let presses = sources::presses(&clip);
    assert_eq!(presses.len(), 1);
    assert!(!presses[0].is_empty());
    assert!(
        presses[0].iter().any(|v| v.abs() > 0.1),
        "the press is in the cut"
    );
}

#[test]
fn the_press_detector_finds_separate_presses_and_merges_ones_closer_than_60_ms() {
    let mut clip = floor();
    click(&mut clip, 10_000);
    click(&mut clip, 40_000);
    click(&mut clip, 70_000);
    assert_eq!(sources::presses(&clip).len(), 3);

    let mut close = floor();
    click(&mut close, 10_000);
    click(&mut close, 10_000 + 1_000); // 23 ms later
    assert_eq!(sources::presses(&close).len(), 1);
}

#[test]
fn the_press_detector_is_deterministic() {
    let mut clip = floor();
    click(&mut clip, 10_000);
    click(&mut clip, 40_000);
    assert_eq!(sources::presses(&clip), sources::presses(&clip));
}

#[test]
fn the_press_detector_skips_presses_that_clip() {
    let mut clip = floor();
    click(&mut clip, 10_000);
    click(&mut clip, 40_000);
    for v in clip.iter_mut().skip(40_000).take(1_500) {
        *v = v.signum();
    }
    assert_eq!(
        sources::presses(&clip).len(),
        1,
        "the clipped press is skipped"
    );
}

#[test]
fn a_pack_whose_every_press_clips_is_still_complete_from_synthesis() {
    let dir = Temp::new("src-clipped");
    let mut clip = floor();
    for at in [10_000, 40_000, 70_000] {
        click(&mut clip, at);
        for v in clip.iter_mut().skip(at).take(1_500) {
            *v = v.signum() * 1.0;
        }
    }
    let samples: Vec<i16> = clip
        .iter()
        .map(|v| (v.clamp(-1.0, 1.0) * 32_767.0) as i16)
        .collect();
    fs::write(
        dir.0.join("loud.wav"),
        wav_file(1, 1, 44_100, 16, &[], &pcm16(&samples), None),
    )
    .unwrap();
    write_list(&dir.0, &[entry("loud.wav", "CC0-1.0")]);
    let out = Temp::new("out-clipped");
    generate(&out.0, &dir.0).unwrap();
    let files = tree(&out.0);
    for id in IDS {
        assert!(files.contains_key(&format!("{id}/pack.json")), "{id}");
    }
    // Every pack still has its full set of default files.
    let defaults = files
        .keys()
        .filter(|k| k.starts_with("thocky-linear/down/default-"))
        .count();
    assert!(defaults >= 8, "{defaults} default files");
}
