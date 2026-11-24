//! Tests for the app icon generator (feature 13, plan task 7: AC20, AC21).
//!
//! Interfaces assumed from the plan, in `main.rs` (the parent module):
//!
//! ```ignore
//! /// The drawing at `size` pixels square, RGBA8, rows top to bottom. Sizes
//! /// of 32 and below get the simplified drawing (three bars, no highlight).
//! pub fn render(size: u32) -> Vec<u8>;
//! /// Writes into `out` (created if missing): `icon.png` (1024),
//! /// `32x32.png`, `128x128.png`, `128x128@2x.png` (256), `icon.ico`
//! /// (16, 24, 32, 48, 64, 256, each entry a PNG) and the folder
//! /// `icon.iconset/` with `icon_<n>x<n>.png` and `icon_<n>x<n>@2x.png`
//! /// for n = 16, 32, 128, 256, 512. No `.icns` (macOS `iconutil` packs it).
//! pub fn generate(out: &Path) -> io::Result<()>;
//! ```
//!
//! The target also depends on the `png` crate (a dev-dependency), used here
//! to decode the generated and committed files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{generate, render};

const FILES: [(&str, u32); 4] = [
    ("icon.png", 1024),
    ("32x32.png", 32),
    ("128x128.png", 128),
    ("128x128@2x.png", 256),
];

const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// (file name, pixel size) of the ten files `iconutil` expects.
const ICONSET: [(&str, u32); 10] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
];

/// A temporary folder removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "mks-appicons-{}-{}-{}",
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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("icons")
}

fn generated(label: &str) -> (Temp, PathBuf) {
    let tmp = Temp::new(label);
    let out = tmp.0.join("icons");
    generate(&out).unwrap();
    (tmp, out)
}

/// An RGBA8 image.
struct Image {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Image {
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.w + x) * 4) as usize;
        self.rgba[at..at + 4].try_into().unwrap()
    }

    fn corners(&self) -> [[u8; 4]; 4] {
        [
            self.px(0, 0),
            self.px(self.w - 1, 0),
            self.px(0, self.h - 1),
            self.px(self.w - 1, self.h - 1),
        ]
    }
}

fn decode(bytes: &[u8]) -> Image {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .expect("a valid PNG");
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).expect("decodable");
    assert_eq!(info.color_type, png::ColorType::Rgba, "RGBA");
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "8 bits");
    buf.truncate(info.buffer_size());
    Image {
        w: info.width,
        h: info.height,
        rgba: buf,
    }
}

fn rendered(size: u32) -> Image {
    let rgba = render(size);
    assert_eq!(rgba.len(), (size * size * 4) as usize, "size {size}");
    Image {
        w: size,
        h: size,
        rgba,
    }
}

/// The entries of an `.ico` as (width in pixels, image bytes).
fn ico_entries(ico: &[u8]) -> Vec<(u32, Vec<u8>)> {
    assert_eq!(&ico[..4], &[0, 0, 1, 0], "an icon file");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    (0..count)
        .map(|i| {
            let e = 6 + i * 16;
            let w = if ico[e] == 0 { 256 } else { u32::from(ico[e]) };
            let len = u32::from_le_bytes(ico[e + 8..e + 12].try_into().unwrap()) as usize;
            let at = u32::from_le_bytes(ico[e + 12..e + 16].try_into().unwrap()) as usize;
            (w, ico[at..at + len].to_vec())
        })
        .collect()
}

fn is_white(p: [u8; 4]) -> bool {
    p[3] >= 200 && p[0] >= 200 && p[1] >= 200 && p[2] >= 200
}

/// The row through the middle of the mark: the middle of the rows holding
/// white pixels. The tall bar spans them all and the middle bars are
/// centred on it, so the row crosses every bar.
fn mark_row(img: &Image) -> u32 {
    let rows: Vec<u32> = (0..img.h)
        .filter(|&y| (0..img.w).any(|x| is_white(img.px(x, y))))
        .collect();
    assert!(!rows.is_empty(), "the image has a white mark");
    (rows[0] + rows[rows.len() - 1]) / 2
}

/// The white runs along a row as (first x, last x).
fn white_runs(img: &Image, y: u32) -> Vec<(u32, u32)> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for x in 0..img.w {
        if is_white(img.px(x, y)) {
            match runs.last_mut() {
                Some(run) if run.1 + 1 == x => run.1 = x,
                _ => runs.push((x, x)),
            }
        }
    }
    runs
}

fn bar_count(img: &Image) -> usize {
    white_runs(img, mark_row(img)).len()
}

/// The expected bar count at a pixel size, from the spec.
fn bars_for(size: u32) -> usize {
    if size <= 32 { 3 } else { 5 }
}

// ---- AC20: the files ----

#[test]
fn ac20_the_generator_writes_the_five_files_and_the_iconset() {
    let (_tmp, out) = generated("names");
    let mut found: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    found.sort();
    let mut expected: Vec<String> = FILES.iter().map(|(n, _)| n.to_string()).collect();
    expected.push("icon.ico".into());
    expected.push("icon.iconset".into());
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn ac20_each_png_has_the_size_its_name_says() {
    let (_tmp, out) = generated("sizes");
    for (name, size) in FILES {
        let img = decode(&fs::read(out.join(name)).unwrap());
        assert_eq!((img.w, img.h), (size, size), "{name}");
    }
}

#[test]
fn ac20_the_iconset_holds_the_ten_png_sizes_iconutil_expects() {
    let (_tmp, out) = generated("iconset");
    let set = out.join("icon.iconset");
    let mut found: Vec<String> = fs::read_dir(&set)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    found.sort();
    let mut expected: Vec<String> = ICONSET.iter().map(|(n, _)| n.to_string()).collect();
    expected.sort();
    assert_eq!(found, expected);
    for (name, size) in ICONSET {
        let img = decode(&fs::read(set.join(name)).unwrap());
        assert_eq!((img.w, img.h), (size, size), "{name}");
    }
}

#[test]
fn ac20_the_ico_holds_at_least_the_six_sizes_each_a_png_of_that_size() {
    let (_tmp, out) = generated("ico");
    let entries = ico_entries(&fs::read(out.join("icon.ico")).unwrap());
    for want in ICO_SIZES {
        let (_, bytes) = entries
            .iter()
            .find(|(w, _)| *w == want)
            .unwrap_or_else(|| panic!("no {want} pixel entry"));
        let img = decode(bytes);
        assert_eq!((img.w, img.h), (want, want), "the {want} entry");
    }
}

#[test]
fn ac20_every_image_has_transparent_corners() {
    let (_tmp, out) = generated("corners");
    let mut all: Vec<(String, Image)> = Vec::new();
    for (name, _) in FILES {
        all.push((name.into(), decode(&fs::read(out.join(name)).unwrap())));
    }
    for (name, _) in ICONSET {
        let bytes = fs::read(out.join("icon.iconset").join(name)).unwrap();
        all.push((format!("iconset/{name}"), decode(&bytes)));
    }
    for (w, bytes) in ico_entries(&fs::read(out.join("icon.ico")).unwrap()) {
        all.push((format!("ico {w}"), decode(&bytes)));
    }
    for (name, img) in all {
        for corner in img.corners() {
            assert_eq!(corner[3], 0, "{name}: a corner is not transparent");
        }
    }
}

#[test]
fn ac20_the_margin_leaves_the_edge_midpoints_transparent_too() {
    // About a tenth of the canvas is empty on each side, so the keycap is
    // the size of other Dock icons.
    let img = rendered(1024);
    for (x, y) in [(512, 0), (512, 1023), (0, 512), (1023, 512), (512, 40)] {
        assert_eq!(img.px(x, y)[3], 0, "({x}, {y}) is outside the keycap");
    }
    let filled = |y: u32| (0..1024).any(|x| img.px(x, y)[3] > 0);
    let top = (0..1024).find(|&y| filled(y)).unwrap();
    let bottom = (0..1024).rev().find(|&y| filled(y)).unwrap();
    assert!(
        (60..=140).contains(&top),
        "the top margin is about a tenth: {top}"
    );
    assert!(
        (1023 - 140..=1023 - 60).contains(&bottom),
        "the bottom margin is about a tenth: {}",
        1023 - bottom
    );
}

#[test]
fn ac20_generating_twice_is_byte_identical() {
    let (_a, first) = generated("twice-a");
    let (_b, second) = generated("twice-b");
    for (name, _) in FILES {
        assert_eq!(
            fs::read(first.join(name)).unwrap(),
            fs::read(second.join(name)).unwrap(),
            "{name}"
        );
    }
    assert_eq!(
        fs::read(first.join("icon.ico")).unwrap(),
        fs::read(second.join("icon.ico")).unwrap()
    );
    for (name, _) in ICONSET {
        assert_eq!(
            fs::read(first.join("icon.iconset").join(name)).unwrap(),
            fs::read(second.join("icon.iconset").join(name)).unwrap(),
            "iconset/{name}"
        );
    }
}

#[test]
fn ac20_generating_into_an_empty_folder_matches_the_committed_files() {
    let (_tmp, out) = generated("committed");
    for name in [
        "icon.png",
        "32x32.png",
        "128x128.png",
        "128x128@2x.png",
        "icon.ico",
    ] {
        let want = fs::read(committed().join(name))
            .unwrap_or_else(|e| panic!("{name} is not committed: {e}"));
        assert_eq!(
            fs::read(out.join(name)).unwrap(),
            want,
            "{name} is out of date: run `npm run make-app-icons`"
        );
    }
}

#[test]
fn ac20_the_png_files_are_compressed() {
    // Stored (uncompressed) it would be four megabytes.
    let (_tmp, out) = generated("compressed");
    let size = fs::metadata(out.join("icon.png")).unwrap().len();
    assert!(size < 400_000, "icon.png is {size} bytes");
}

// ---- AC21: the drawing ----

#[test]
fn ac21_the_centre_of_the_tallest_bar_is_white_and_the_body_is_the_accent_blue() {
    let img = rendered(1024);
    let row = mark_row(&img);
    // The tallest bar is the one on the centre line: the long white run
    // down the middle column.
    let column = |x: u32| -> Vec<u32> { (0..1024).filter(|&y| is_white(img.px(x, y))).collect() };
    let tall = column(511);
    let tall_alt = column(512);
    let tall = if tall.len() > tall_alt.len() {
        tall
    } else {
        tall_alt
    };
    assert!(
        tall.len() >= 100,
        "the middle bar is tall: {} white pixels",
        tall.len()
    );
    let middle = (tall[0] + tall[tall.len() - 1]) / 2;
    assert_eq!(middle, row, "the tall bar's centre is the mark's middle");
    let p = img.px(511, middle);
    let q = img.px(512, middle);
    assert!(
        is_white(p) || is_white(q),
        "centre of the tallest bar: {p:?} {q:?}"
    );
    assert!(
        middle < 560,
        "the key top sits a little above centre: {middle}"
    );

    let blue = |p: [u8; 4]| {
        p[3] == 255
            && p[0].abs_diff(0x25) <= 2
            && p[1].abs_diff(0x63) <= 2
            && p[2].abs_diff(0xeb) <= 2
    };
    let count = (0..1024)
        .flat_map(|y| (0..1024).map(move |x| (x, y)))
        .filter(|&(x, y)| blue(img.px(x, y)))
        .count();
    assert!(
        count > 1024 * 1024 / 50,
        "a visible part of the body is #2563eb: {count} pixels"
    );
}

#[test]
fn ac21_the_1024_drawing_is_symmetrical_left_to_right() {
    let img = rendered(1024);
    let mut off = 0usize;
    let mut total = 0usize;
    for y in 0..1024 {
        for x in 0..512 {
            let (a, b) = (img.px(x, y), img.px(1023 - x, y));
            if a[3] == 0 && b[3] == 0 {
                continue;
            }
            total += 1;
            if (0..4).any(|c| a[c].abs_diff(b[c]) > 16) {
                off += 1;
            }
        }
    }
    assert!(total > 100_000, "there is a drawing: {total}");
    assert!(
        off * 100 <= total,
        "{off} of {total} mirrored pixels differ by more than 16"
    );
}

#[test]
fn ac21_five_bars_above_32_pixels_and_three_at_32_and_below() {
    for size in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
        let img = rendered(size);
        assert_eq!(bar_count(&img), bars_for(size), "bars at {size} pixels");
    }
}

#[test]
fn ac21_at_16_and_32_pixels_the_bars_are_separate_along_the_middle_row() {
    for size in [16u32, 32] {
        let img = rendered(size);
        let y = mark_row(&img);
        let pattern: String = (0..size)
            .map(|x| if is_white(img.px(x, y)) { 'W' } else { '.' })
            .collect();
        let squeezed: String = pattern
            .chars()
            .fold(String::new(), |mut s, c| {
                if !s.ends_with(c) {
                    s.push(c);
                }
                s
            })
            .trim_matches('.')
            .to_string();
        assert_eq!(squeezed, "W.W.W", "row {y} at {size}: {pattern}");
    }
}

#[test]
fn ac21_the_small_bars_sit_on_whole_pixels_so_they_stay_crisp() {
    // No half-white pixel at the edge of a bar on the middle row: every
    // pixel of the row is white, or clearly not.
    for size in [16u32, 24, 32] {
        let img = rendered(size);
        let y = mark_row(&img);
        for x in 0..size {
            let p = img.px(x, y);
            let blueish = p[0] < 120;
            assert!(
                is_white(p) || blueish || p[3] < 200,
                "{size}: ({x}, {y}) is {p:?}"
            );
        }
        // Bars and the gaps between them are whole pixels at least one wide.
        for (a, b) in white_runs(&img, y).windows(2).map(|w| (w[0], w[1])) {
            assert!(b.0 > a.1 + 1, "{size}: bars touch");
        }
    }
}

#[test]
fn ac21_the_white_bars_have_3_to_1_contrast_against_the_key_top() {
    fn linear(c: u8) -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    fn luminance(p: [u8; 4]) -> f64 {
        0.2126 * linear(p[0]) + 0.7152 * linear(p[1]) + 0.0722 * linear(p[2])
    }
    for size in [16u32, 32, 128, 1024] {
        let img = rendered(size);
        let y = mark_row(&img);
        let runs = white_runs(&img, y);
        // The gap between the first two bars is key top.
        let gap = img.px((runs[0].1 + runs[1].0) / 2, y);
        assert_eq!(gap[3], 255, "{size}: the key top is opaque");
        let ratio = 1.05 / (luminance(gap) + 0.05);
        assert!(
            ratio >= 3.0,
            "{size}: contrast {ratio:.2} for key top {gap:?}"
        );
    }
}

#[test]
fn ac21_no_bar_touches_the_edge_of_the_key_top() {
    for size in [16u32, 32, 64, 256, 1024] {
        let img = rendered(size);
        let y = mark_row(&img);
        let runs = white_runs(&img, y);
        let gap = img.px((runs[0].1 + runs[1].0) / 2, y);
        let like_key_top = |p: [u8; 4]| p[3] == 255 && (0..3).all(|c| p[c].abs_diff(gap[c]) <= 48);
        let first = runs[0].0;
        let last = runs[runs.len() - 1].1;
        assert!(
            like_key_top(img.px(first - 1, y)),
            "{size}: left of the bars"
        );
        assert!(
            like_key_top(img.px(last + 1, y)),
            "{size}: right of the bars"
        );
        // The tall bar has key top above and below it.
        let mid = runs[runs.len() / 2];
        let x = (mid.0 + mid.1) / 2;
        let tall: Vec<u32> = (0..size).filter(|&yy| is_white(img.px(x, yy))).collect();
        assert!(
            like_key_top(img.px(x, tall[0] - 1)),
            "{size}: above the tall bar"
        );
        assert!(
            like_key_top(img.px(x, tall[tall.len() - 1] + 1)),
            "{size}: below the tall bar"
        );
    }
}

#[test]
fn ac21_the_bars_are_about_as_wide_as_the_gaps_and_fill_about_half_the_key_top() {
    let img = rendered(1024);
    let y = mark_row(&img);
    let runs = white_runs(&img, y);
    assert_eq!(runs.len(), 5);
    let widths: Vec<u32> = runs.iter().map(|r| r.1 - r.0 + 1).collect();
    let gaps: Vec<u32> = runs.windows(2).map(|w| w[1].0 - w[0].1 - 1).collect();
    for w in &widths {
        for g in &gaps {
            let ratio = f64::from(*w) / f64::from(*g);
            assert!((0.6..=1.7).contains(&ratio), "bar {w} against gap {g}");
        }
    }
    // The key top is the opaque run of key-top colour around the bars.
    let gap = img.px((runs[0].1 + runs[1].0) / 2, y);
    let like = |x: u32| {
        let p = img.px(x, y);
        (p[3] == 255 && (0..3).all(|c| p[c].abs_diff(gap[c]) <= 48)) || is_white(p)
    };
    let mut left = runs[0].0;
    while left > 0 && like(left - 1) {
        left -= 1;
    }
    let mut right = runs[4].1;
    while right < 1023 && like(right + 1) {
        right += 1;
    }
    let top_width = f64::from(right - left + 1);
    let mark_width = f64::from(runs[4].1 - runs[0].0 + 1);
    let share = mark_width / top_width;
    assert!(
        (0.35..=0.65).contains(&share),
        "the mark is {share:.2} of the key top's width"
    );
}

#[test]
fn ac21_heights_are_low_middle_tall_middle_low() {
    let img = rendered(1024);
    let y = mark_row(&img);
    let heights: Vec<u32> = white_runs(&img, y)
        .iter()
        .map(|(a, b)| {
            let x = (a + b) / 2;
            (0..1024).filter(|&yy| is_white(img.px(x, yy))).count() as u32
        })
        .collect();
    assert_eq!(heights.len(), 5);
    assert!(
        heights[0] < heights[1] && heights[1] < heights[2],
        "{heights:?}"
    );
    assert!(
        heights[4] < heights[3] && heights[3] < heights[2],
        "{heights:?}"
    );
    assert!(heights[0].abs_diff(heights[4]) <= 2, "{heights:?}");
    assert!(heights[1].abs_diff(heights[3]) <= 2, "{heights:?}");
}

#[test]
fn ac21_the_iconset_uses_the_small_drawing_for_both_32_pixel_files() {
    let (_tmp, out) = generated("small");
    let set = out.join("icon.iconset");
    for name in ["icon_16x16.png", "icon_16x16@2x.png", "icon_32x32.png"] {
        let img = decode(&fs::read(set.join(name)).unwrap());
        assert_eq!(bar_count(&img), 3, "{name}");
    }
    for name in [
        "icon_32x32@2x.png",
        "icon_128x128.png",
        "icon_512x512@2x.png",
    ] {
        let img = decode(&fs::read(set.join(name)).unwrap());
        assert_eq!(bar_count(&img), 5, "{name}");
    }
}

#[test]
fn ac21_the_generated_files_hold_the_same_drawing_as_render() {
    let (_tmp, out) = generated("render");
    for (name, size) in FILES {
        let img = decode(&fs::read(out.join(name)).unwrap());
        assert_eq!(img.rgba, render(size), "{name}");
    }
    for (w, bytes) in ico_entries(&fs::read(out.join("icon.ico")).unwrap()) {
        assert_eq!(decode(&bytes).rgba, render(w), "ico {w}");
    }
}

#[test]
fn the_icon_reads_on_white_grey_and_near_black() {
    // The keycap fills its own silhouette: the pixels at its centre are
    // opaque, so the drawing does not depend on the background.
    let img = rendered(1024);
    for (x, y) in [(512, 300), (512, 700), (250, 600), (780, 600)] {
        assert_eq!(img.px(x, y)[3], 255, "({x}, {y})");
    }
}
