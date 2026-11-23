//! Tests for the tray icon generator (feature 07, plan task 3: AC1 assets).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::png::{adler32, crc32, encode};
use super::{generate, icons};

const NAMES: [&str; 4] = ["mac.png", "mac-off.png", "win.png", "win-off.png"];

/// A temporary folder removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "mks-trayicons-{}-{}-{}",
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

fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("icons")
        .join("tray")
}

fn generated(label: &str) -> (Temp, PathBuf) {
    let tmp = Temp::new(label);
    let out = tmp.0.join("tray");
    generate(&out).unwrap();
    (tmp, out)
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}

/// The chunks of a PNG as (kind, data, stored crc), after the signature.
fn chunks(png: &[u8]) -> Vec<([u8; 4], Vec<u8>, u32)> {
    let mut out = Vec::new();
    let mut at = 8;
    while at < png.len() {
        let len = be32(&png[at..at + 4]) as usize;
        let kind: [u8; 4] = png[at + 4..at + 8].try_into().unwrap();
        let data = png[at + 8..at + 8 + len].to_vec();
        let crc = be32(&png[at + 8 + len..at + 12 + len]);
        out.push((kind, data, crc));
        at += 12 + len;
    }
    assert_eq!(at, png.len(), "no trailing bytes");
    out
}

#[test]
fn the_generator_makes_the_four_named_files_and_nothing_else() {
    let (_tmp, out) = generated("names");
    let mut found: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    found.sort();
    let mut expected: Vec<String> = NAMES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn each_file_has_the_png_signature_and_the_right_size_in_ihdr() {
    let (_tmp, out) = generated("ihdr");
    for (name, size) in [
        ("mac.png", 44),
        ("mac-off.png", 44),
        ("win.png", 32),
        ("win-off.png", 32),
    ] {
        let bytes = fs::read(out.join(name)).unwrap();
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        let all = chunks(&bytes);
        assert_eq!(&all[0].0, b"IHDR");
        assert_eq!(all[0].1.len(), 13);
        assert_eq!(be32(&all[0].1[0..4]), size, "{name} width");
        assert_eq!(be32(&all[0].1[4..8]), size, "{name} height");
        assert_eq!(all[0].1[8], 8, "8 bits");
        assert_eq!(all[0].1[9], 6, "RGBA");
        assert_eq!(&all.last().unwrap().0, b"IEND");
    }
}

#[test]
fn every_chunk_crc_is_correct() {
    let (_tmp, out) = generated("crc");
    for name in NAMES {
        let bytes = fs::read(out.join(name)).unwrap();
        for (kind, data, stored) in chunks(&bytes) {
            let mut body = kind.to_vec();
            body.extend_from_slice(&data);
            assert_eq!(crc32(&body), stored, "{name} {kind:?}");
        }
    }
}

#[test]
fn the_off_icon_differs_from_the_normal_one_in_both_pairs() {
    let (_tmp, out) = generated("off");
    let read = |n: &str| fs::read(out.join(n)).unwrap();
    assert_ne!(read("mac.png"), read("mac-off.png"));
    assert_ne!(read("win.png"), read("win-off.png"));
    let all = icons();
    let pixels = |n: &str| all.iter().find(|i| i.name == n).unwrap().rgba.clone();
    assert_ne!(pixels("mac.png"), pixels("mac-off.png"));
    assert_ne!(pixels("win.png"), pixels("win-off.png"));
}

#[test]
fn icons_carry_the_declared_sizes_and_pixel_counts() {
    let all = icons();
    assert_eq!(all.len(), 4);
    for icon in &all {
        let expected = if icon.name.starts_with("mac") { 44 } else { 32 };
        assert_eq!(icon.size, expected, "{}", icon.name);
        assert_eq!(icon.rgba.len(), (icon.size * icon.size * 4) as usize);
        assert!(
            icon.rgba.chunks(4).any(|p| p[3] > 0),
            "{} is not empty",
            icon.name
        );
    }
}

#[test]
fn generating_twice_is_byte_identical() {
    let (_a, first) = generated("twice-a");
    let (_b, second) = generated("twice-b");
    for name in NAMES {
        assert_eq!(
            fs::read(first.join(name)).unwrap(),
            fs::read(second.join(name)).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn generating_into_an_empty_folder_matches_the_committed_files() {
    let (_tmp, out) = generated("committed");
    for name in NAMES {
        let want = fs::read(committed().join(name))
            .unwrap_or_else(|e| panic!("{name} is not committed: {e}"));
        assert_eq!(
            fs::read(out.join(name)).unwrap(),
            want,
            "{name} is out of date: run `npm run make-tray-icons`"
        );
    }
}

#[test]
fn the_mac_icons_are_black_only_so_they_work_as_template_images() {
    for icon in icons().iter().filter(|i| i.name.starts_with("mac")) {
        for (n, p) in icon.rgba.chunks(4).enumerate() {
            if p[3] > 0 {
                assert_eq!(&p[..3], &[0, 0, 0], "{} pixel {n}", icon.name);
            }
        }
    }
}

#[test]
fn crc32_known_answers() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    assert_eq!(crc32(b"IEND"), 0xae42_6082);
}

#[test]
fn adler32_known_answers() {
    assert_eq!(adler32(b""), 1);
    assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    assert_eq!(adler32(b"a"), 0x0062_0062);
}

#[test]
fn encode_makes_a_valid_png_for_a_tiny_image() {
    let rgba = [255, 0, 0, 255, 0, 255, 0, 128];
    let png = encode(2, 1, &rgba);
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let all = chunks(&png);
    let kinds: Vec<&[u8; 4]> = all.iter().map(|c| &c.0).collect();
    assert_eq!(kinds, [b"IHDR", b"IDAT", b"IEND"]);
    // zlib header, one final stored block holding filter byte + 8 bytes.
    let idat = &all[1].1;
    assert_eq!(&idat[..2], &[0x78, 0x01]);
    assert_eq!(idat[2], 1, "final stored block");
    assert_eq!(u16::from_le_bytes([idat[3], idat[4]]), 9);
    assert_eq!(
        &idat[7..],
        &{
            let mut raw = vec![0];
            raw.extend_from_slice(&rgba);
            let mut v = raw[..].to_vec();
            v.truncate(9);
            let sum = adler32(&v).to_be_bytes();
            let mut full = v;
            full.extend_from_slice(&sum);
            full
        }[..]
    );
}
