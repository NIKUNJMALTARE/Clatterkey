//! Generates the app icon: `npm run make-app-icons`.
//!
//! The icon is a keycap carrying a sound-wave mark, drawn here from code
//! alone, so it belongs to the project. The output is deterministic and
//! committed in `src-tauri/icons/`; the app, the tests and CI never run
//! this. Run it again after changing the drawing and commit the result.
//!
//! There are two drawings. Above 32 pixels: an accent-blue key with a
//! darker lower edge (the key's side), a lighter key top a little above
//! centre with one soft highlight, and five white bars on it, low, middle,
//! tall, middle, low. At 32 pixels and below the drawing is made again,
//! not shrunk: three bars and no highlight. At every size the edges of the
//! key and of the bars sit on whole pixels, so the bars stay separate and
//! crisp down to 16 pixels.
//!
//! Written here: `icon.png` (1024), `32x32.png`, `128x128.png`,
//! `128x128@2x.png`, `icon.ico` and, for macOS, the folder `icon.iconset`,
//! which `scripts/make-app-icons.sh` packs into `icon.icns` with the
//! system's `iconutil`.
//!
//! This is a development tool. It is an example target, so none of it is
//! compiled into the app.

mod ico;

#[cfg(test)]
mod tests;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Samples per pixel along each axis, for smooth edges.
const SAMPLES: u32 = 4;

/// The largest size that gets the small drawing.
const SMALL_UP_TO: u32 = 32;

type Rgb = [f64; 3];

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    [r as f64, g as f64, b as f64]
}

/// The window's accent colour (`--color-accent`).
const BODY: Rgb = rgb(0x25, 0x63, 0xeb);
/// The key's side, seen below the face.
const SIDE: Rgb = rgb(0x1a, 0x46, 0xb0);
/// The key top: lighter than the body, and dark enough that the white
/// bars keep a contrast of more than 3 to 1 against it.
const TOP: Rgb = rgb(0x44, 0x80, 0xf5);
/// How much lighter the very top of the key top is, per channel.
const HIGHLIGHT: f64 = 14.0;
const WHITE: Rgb = rgb(0xff, 0xff, 0xff);

/// A rectangle in pixels, edges on whole pixels, with rounded corners.
#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    radius: f64,
}

impl Rect {
    fn holds(&self, x: f64, y: f64) -> bool {
        if x < self.x0 || x >= self.x1 || y < self.y0 || y >= self.y1 {
            return false;
        }
        let r = self.radius;
        let dx = (self.x0 + r - x).max(x - (self.x1 - r));
        let dy = (self.y0 + r - y).max(y - (self.y1 - r));
        dx <= 0.0 || dy <= 0.0 || dx * dx + dy * dy <= r * r
    }
}

/// The drawing laid out for one size, in pixels.
struct Layout {
    /// The whole key: the silhouette.
    key: Rect,
    /// The face of the key; below it the side shows.
    face: Rect,
    top: Rect,
    bars: Vec<Rect>,
    highlight: bool,
}

/// `value` of the canvas, on a whole pixel.
fn snap(size: u32, value: f64) -> f64 {
    (value * f64::from(size)).round()
}

/// The nearest whole number to `wanted` that is at least `least` and odd
/// or even as `like` is, so that something of that length is centred on
/// whole pixels inside something of length `like`.
fn matching(wanted: f64, least: f64, like: f64) -> f64 {
    let mut length = wanted.round().max(least);
    if (length - like).rem_euclid(2.0) != 0.0 {
        length += 1.0;
    }
    length
}

fn layout(size: u32) -> Layout {
    let s = f64::from(size);
    let small = size <= SMALL_UP_TO;

    // About a tenth of the canvas is left empty on each side, as other
    // macOS icons do. The left and right edges mirror each other.
    let margin = snap(size, 0.098).max(1.0);
    let key = Rect {
        x0: margin,
        y0: margin,
        x1: s - margin,
        y1: s - margin,
        radius: 0.186 * s,
    };
    let lip = snap(size, 0.043).max(1.0);
    let face = Rect {
        y1: key.y1 - lip,
        ..key
    };
    let inset = snap(size, 0.186);
    let top = Rect {
        x0: inset,
        y0: snap(size, 0.146),
        x1: s - inset,
        y1: snap(size, 0.752),
        radius: 0.127 * s,
    };

    // The bars, centred on the key top. The canvas is even, so the bar in
    // the middle is an even number of pixels wide.
    let (count, width, gap) = match size {
        16 => (3, 2.0, 1.0),
        17..=24 => (3, 2.0, 1.0),
        25..=SMALL_UP_TO => (3, 2.0, 2.0),
        _ => {
            let width = 2.0 * (0.018 * s).round().max(1.0);
            (5, width, width)
        }
    };
    let top_height = top.y1 - top.y0;
    let centre_y = (top.y0 + top.y1) / 2.0;
    let heights: Vec<f64> = if small {
        vec![0.26, 0.42, 0.26]
    } else {
        vec![0.10, 0.20, 0.31, 0.20, 0.10]
    }
    .into_iter()
    .map(|share| matching(share * s, 2.0, top_height))
    .collect();
    // At 16 pixels the outer bars are one pixel wide: three bars of two
    // would fill the key top.
    let widths: Vec<f64> = (0..count)
        .map(|i| {
            if size == 16 && i != count / 2 {
                1.0
            } else {
                width
            }
        })
        .collect();
    let total: f64 = widths.iter().sum::<f64>() + gap * (count as f64 - 1.0);
    let mut x = s / 2.0 - total / 2.0;
    let mut bars = Vec::with_capacity(count);
    for (width, height) in widths.iter().zip(&heights) {
        bars.push(Rect {
            x0: x,
            y0: centre_y - height / 2.0,
            x1: x + width,
            y1: centre_y + height / 2.0,
            // Round ends where there are pixels to round them with.
            radius: if small { 0.0 } else { width / 2.0 },
        });
        x += width + gap;
    }

    Layout {
        key,
        face,
        top,
        bars,
        highlight: !small,
    }
}

/// The colour at a point, in pixels, if anything is drawn there.
fn sample(layout: &Layout, x: f64, y: f64) -> Option<Rgb> {
    if !layout.key.holds(x, y) {
        return None;
    }
    if layout.bars.iter().any(|bar| bar.holds(x, y)) {
        return Some(WHITE);
    }
    if layout.top.holds(x, y) {
        if !layout.highlight {
            return Some(TOP);
        }
        // Lightest at the upper edge, gone a third of the way down.
        let depth = (y - layout.top.y0) / (layout.top.y1 - layout.top.y0);
        let lift = HIGHLIGHT * (1.0 - depth / 0.33).max(0.0);
        return Some([TOP[0] + lift, TOP[1] + lift, (TOP[2] + lift).min(255.0)]);
    }
    if layout.face.holds(x, y) {
        return Some(BODY);
    }
    Some(SIDE)
}

/// The drawing at `size` pixels square, RGBA8, rows top to bottom. Sizes
/// of 32 and below get the small drawing.
pub fn render(size: u32) -> Vec<u8> {
    let layout = layout(size);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let per_pixel = f64::from(SAMPLES);
    for py in 0..size {
        for px in 0..size {
            let (mut sum, mut hits) = ([0.0; 3], 0.0);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = f64::from(px) + (f64::from(sx) + 0.5) / per_pixel;
                    let y = f64::from(py) + (f64::from(sy) + 0.5) / per_pixel;
                    if let Some(colour) = sample(&layout, x, y) {
                        for (total, channel) in sum.iter_mut().zip(colour) {
                            *total += channel;
                        }
                        hits += 1.0;
                    }
                }
            }
            for total in sum {
                let channel = if hits > 0.0 { total / hits } else { 0.0 };
                rgba.push(channel.round() as u8);
            }
            rgba.push((hits / (per_pixel * per_pixel) * 255.0).round() as u8);
        }
    }
    rgba
}

/// The drawing at `size` pixels as a compressed PNG.
fn png_of(size: u32) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer
            .write_image_data(&render(size))
            .map_err(io::Error::other)?;
    }
    Ok(bytes)
}

/// The PNG files beside the `.ico`: (name, pixels).
const FILES: [(&str, u32); 4] = [
    ("icon.png", 1024),
    ("32x32.png", 32),
    ("128x128.png", 128),
    ("128x128@2x.png", 256),
];

/// The sizes in `icon.ico`.
const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// The folder `iconutil` packs into `icon.icns`.
pub const ICONSET: &str = "icon.iconset";

/// The points sizes of an iconset; each is written at 1x and at 2x.
const ICONSET_POINTS: [u32; 5] = [16, 32, 128, 256, 512];

/// Writes the icon files and the iconset into `out`, creating it if
/// needed.
pub fn generate(out: &Path) -> io::Result<()> {
    fs::create_dir_all(out)?;
    for (name, size) in FILES {
        fs::write(out.join(name), png_of(size)?)?;
    }
    let mut entries = Vec::with_capacity(ICO_SIZES.len());
    for size in ICO_SIZES {
        entries.push((size, png_of(size)?));
    }
    fs::write(out.join("icon.ico"), ico::encode(&entries))?;

    let set = out.join(ICONSET);
    fs::create_dir_all(&set)?;
    for points in ICONSET_POINTS {
        fs::write(
            set.join(format!("icon_{points}x{points}.png")),
            png_of(points)?,
        )?;
        fs::write(
            set.join(format!("icon_{points}x{points}@2x.png")),
            png_of(points * 2)?,
        )?;
    }
    Ok(())
}

/// Generates into the build folder, then copies the files the app is
/// bundled with into `icons/`. The iconset stays in the build folder.
fn run() -> io::Result<(PathBuf, PathBuf)> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let work = manifest.join("target").join("app-icons");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    generate(&work)?;
    let icons = manifest.join("icons");
    for (name, _) in FILES {
        fs::copy(work.join(name), icons.join(name))?;
    }
    fs::copy(work.join("icon.ico"), icons.join("icon.ico"))?;
    Ok((icons, work.join(ICONSET)))
}

fn main() -> ExitCode {
    match run() {
        Ok((icons, iconset)) => {
            println!("wrote the app icons to {}", icons.display());
            println!("wrote the iconset to {}", iconset.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not write the app icons: {error}");
            ExitCode::FAILURE
        }
    }
}
