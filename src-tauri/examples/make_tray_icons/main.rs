//! Generates the tray icons: `npm run make-tray-icons`.
//!
//! One keycap shape (a rounded square carrying three upright bars, the
//! sound-wave mark of the app icon reduced to tray size) is drawn here
//! from code alone, so the icons belong to the project. The
//! output is deterministic and committed in `src-tauri/icons/tray/`; the
//! app, the tests and CI never run this. Run it again after changing the
//! drawing and commit the result.
//!
//! - `mac.png`, `mac-off.png`: 44×44, black with alpha. macOS uses them as
//!   template images and colours them for the menu bar, so the bars are
//!   cut out of the key. Off is fainter and struck through.
//! - `win.png`, `win-off.png`: 32×32, the accent blue with white bars, and
//!   the same in grey.
//!
//! This is a development tool. It is an example target, so none of it is
//! compiled into the app.

mod png;

#[cfg(test)]
mod tests;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Samples per pixel along each axis, for smooth edges.
const SAMPLES: u32 = 4;

/// An image before it is encoded.
pub struct Icon {
    pub name: &'static str,
    pub size: u32,
    pub rgba: Vec<u8>,
}

/// A colour with its opacity, each 0 to 1.
#[derive(Clone, Copy)]
struct Paint {
    rgb: [f64; 3],
    alpha: f64,
}

const fn paint(r: u8, g: u8, b: u8, alpha: f64) -> Paint {
    Paint {
        rgb: [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0],
        alpha,
    }
}

/// How one icon is coloured. `bars` of `None` cuts the bars out of the
/// body, which is what a one-colour template image needs.
struct Style {
    body: Paint,
    bars: Option<Paint>,
    /// A diagonal stroke through the icon, for "off".
    strike: Option<Paint>,
}

/// Whether the point is inside a rounded square centred in the unit
/// square, `inset` from its edges, with corner radius `radius`.
fn in_rounded_square(x: f64, y: f64, inset: f64, radius: f64) -> bool {
    let half = 0.5 - inset;
    let (dx, dy) = ((x - 0.5).abs(), (y - 0.5).abs());
    if dx > half || dy > half {
        return false;
    }
    let (cx, cy) = (dx - (half - radius), dy - (half - radius));
    cx <= 0.0 || cy <= 0.0 || cx * cx + cy * cy <= radius * radius
}

/// Whether the point is on the stroke from bottom left to top right.
fn on_strike(x: f64, y: f64) -> bool {
    let distance = (x + y - 1.0).abs() / std::f64::consts::SQRT_2;
    distance <= 0.055 && (0.08..=0.92).contains(&x)
}

/// The three bars of the mark, middle, tall, middle, as (left, right,
/// top, bottom) in pixels. They are laid out on whole pixels for each
/// size, so they stay separate when the system scales the icon down.
fn bars(size: u32) -> [(f64, f64, f64, f64); 3] {
    match size {
        44 => [
            (12.0, 16.0, 17.0, 27.0),
            (20.0, 24.0, 12.0, 32.0),
            (28.0, 32.0, 17.0, 27.0),
        ],
        32 => [
            (8.0, 12.0, 12.0, 20.0),
            (14.0, 18.0, 8.0, 24.0),
            (20.0, 24.0, 12.0, 20.0),
        ],
        _ => panic!("no bars are laid out for {size} pixels"),
    }
}

/// Whether the point (in pixels) is on a bar. The ends are round.
fn on_bar(size: u32, px: f64, py: f64) -> bool {
    bars(size).iter().any(|&(left, right, top, bottom)| {
        if px < left || px >= right || py < top || py >= bottom {
            return false;
        }
        let radius = (right - left) / 2.0;
        let dx = px - (left + radius);
        let dy = (top + radius - py).max(py - (bottom - radius));
        dy <= 0.0 || dx * dx + dy * dy <= radius * radius
    })
}

/// The colour at a point of the unit square, if anything is drawn there.
fn sample(style: &Style, size: u32, x: f64, y: f64) -> Option<Paint> {
    // The bars come first: where they are cut out, they are cut out of
    // the strike as well, so the mark is the same on and off.
    if on_bar(size, x * f64::from(size), y * f64::from(size)) {
        return style.bars;
    }
    if let Some(strike) = style.strike
        && on_strike(x, y)
    {
        return Some(strike);
    }
    if !in_rounded_square(x, y, 0.06, 0.2) {
        return None;
    }
    Some(style.body)
}

/// Draws the keycap at `size` pixels square.
fn draw(size: u32, style: &Style) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let steps = f64::from(size * SAMPLES);
    for py in 0..size {
        for px in 0..size {
            // Average the samples, weighting colour by opacity.
            let (mut rgb, mut alpha) = ([0.0; 3], 0.0);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = (f64::from(px * SAMPLES + sx) + 0.5) / steps;
                    let y = (f64::from(py * SAMPLES + sy) + 0.5) / steps;
                    if let Some(p) = sample(style, size, x, y) {
                        for (sum, channel) in rgb.iter_mut().zip(p.rgb) {
                            *sum += channel * p.alpha;
                        }
                        alpha += p.alpha;
                    }
                }
            }
            for sum in rgb {
                let channel = if alpha > 0.0 { sum / alpha } else { 0.0 };
                rgba.push((channel * 255.0).round() as u8);
            }
            let coverage = alpha / f64::from(SAMPLES * SAMPLES);
            rgba.push((coverage * 255.0).round() as u8);
        }
    }
    rgba
}

/// The four icons.
pub fn icons() -> Vec<Icon> {
    let black = |alpha| paint(0, 0, 0, alpha);
    let white = paint(255, 255, 255, 1.0);
    let styles = [
        (
            "mac.png",
            44,
            Style {
                body: black(1.0),
                bars: None,
                strike: None,
            },
        ),
        (
            "mac-off.png",
            44,
            Style {
                body: black(0.45),
                bars: None,
                strike: Some(black(1.0)),
            },
        ),
        (
            "win.png",
            32,
            Style {
                body: paint(0x25, 0x63, 0xeb, 1.0),
                bars: Some(white),
                strike: None,
            },
        ),
        (
            "win-off.png",
            32,
            Style {
                body: paint(0x8a, 0x8f, 0x98, 1.0),
                bars: Some(paint(0xe4, 0xe6, 0xea, 1.0)),
                strike: None,
            },
        ),
    ];
    styles
        .into_iter()
        .map(|(name, size, style)| Icon {
            name,
            size,
            rgba: draw(size, &style),
        })
        .collect()
}

/// Writes the icons into `out`, creating it if needed.
pub fn generate(out: &Path) -> io::Result<()> {
    fs::create_dir_all(out)?;
    for icon in icons() {
        fs::write(
            out.join(icon.name),
            png::encode(icon.size, icon.size, &icon.rgba),
        )?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("icons")
        .join("tray");
    match generate(&out) {
        Ok(()) => {
            println!("wrote the tray icons to {}", out.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("make-tray-icons: {error}");
            ExitCode::FAILURE
        }
    }
}
