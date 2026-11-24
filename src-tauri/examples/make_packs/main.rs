//! Generates the bundled sound packs: `npm run make-packs`.
//!
//! Every sound is synthesised here from code alone (noise bursts and damped
//! resonances), so the packs belong to the project and are released as CC0.
//! The output is deterministic and committed in `src-tauri/resources/packs/`;
//! the app, the tests and CI never run this. Run it again after changing a
//! recipe and commit the result.
//!
//! This is a development tool. It is an example target, so none of it is
//! compiled into the app.

mod synth;
mod voices;
mod wav;

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use synth::Rng;
use voices::{Kind, Pack, group_gain, recipe};

/// The highest peak any file may have: about -1.4 dBFS, under the -1 dBFS
/// limit after rounding to 16 bits.
const MAX_PEAK: f64 = 0.85;

const AUTHOR: &str = "ClickClak contributors";
const LICENSE: &str = "CC0-1.0";

/// One generated file before it is written.
struct File {
    kind: Kind,
    group: voices::Group,
    /// Relative to the pack folder, with `/`.
    name: String,
    samples: Vec<f64>,
}

fn main() -> ExitCode {
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/packs"));
    match generate(&out) {
        Ok(()) => {
            println!("wrote the sound packs to {}", out.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "could not write the sound packs to {}: {error}",
                out.display()
            );
            ExitCode::FAILURE
        }
    }
}

/// Writes the four packs and the licence file into `out`, replacing any
/// earlier output there. Nothing else in `out` is touched.
pub fn generate(out: &Path) -> io::Result<()> {
    fs::create_dir_all(out)?;

    let mut packs: Vec<(Pack, Vec<File>)> = Pack::ALL
        .iter()
        .map(|&pack| (pack, render_pack(pack)))
        .collect();
    level(&mut packs);

    for (pack, files) in &packs {
        let dir = out.join(pack.id());
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        for file in files {
            let path = dir.join(&file.name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, wav::encode(&file.samples))?;
        }
        fs::write(dir.join("pack.json"), manifest(*pack, files))?;
    }
    fs::write(out.join("LICENSE.md"), LICENSE_TEXT)?;
    Ok(())
}

/// Every file of one pack, each normalised to a peak of 1 and then scaled
/// by its group's relative loudness.
fn render_pack(pack: Pack) -> Vec<File> {
    let mut files = Vec::new();
    for (kind, group, count) in pack.layout() {
        for variation in 1..=count {
            let name = format!("{}/{}-{variation}.wav", kind.name(), group.name());
            let mut rng = Rng::from_name(&format!("{}/{name}", pack.id()));
            let (strikes, length) = recipe(pack, kind, group, &mut rng);
            let mut samples = synth::finish(synth::render(&strikes, length, &mut rng));
            let p = synth::peak(&samples);
            synth::scale(&mut samples, group_gain(pack, kind, group) / p);
            files.push(File {
                kind,
                group,
                name,
                samples,
            });
        }
    }
    files
}

/// Levels the packs against each other: each pack gets one gain, so that the
/// mean RMS of its letter sounds is the same in every pack, chosen as high as
/// possible while no file peaks above `MAX_PEAK`.
fn level(packs: &mut [(Pack, Vec<File>)]) {
    let letters_rms = |files: &[File]| {
        let letters: Vec<f64> = files
            .iter()
            .filter(|f| f.kind == Kind::Down && f.group == voices::Group::Default)
            .map(|f| synth::rms(&f.samples))
            .collect();
        letters.iter().sum::<f64>() / letters.len() as f64
    };
    let max_peak = |files: &[File]| {
        files
            .iter()
            .map(|f| synth::peak(&f.samples))
            .fold(0.0, |m: f64, p| if p > m { p } else { m })
    };
    // The loudest common level every pack can reach without clipping.
    let target = packs
        .iter()
        .map(|(_, files)| letters_rms(files) * MAX_PEAK / max_peak(files))
        .fold(f64::INFINITY, |m, t| if t < m { t } else { m });
    for (_, files) in packs.iter_mut() {
        let gain = target / letters_rms(files);
        for file in files.iter_mut() {
            synth::scale(&mut file.samples, gain);
        }
    }
}

/// The pack's `pack.json`, with fields in a fixed order.
fn manifest(pack: Pack, files: &[File]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{{");
    let _ = writeln!(s, "  \"name\": \"{}\",", pack.name());
    let _ = writeln!(s, "  \"author\": \"{AUTHOR}\",");
    let _ = writeln!(s, "  \"license\": \"{LICENSE}\",");
    let sections: Vec<(Kind, &str)> = vec![(Kind::Down, "keys"), (Kind::Up, "up")];
    let mut written = Vec::new();
    for (kind, key) in sections {
        let groups: Vec<(voices::Group, Vec<&str>)> = pack
            .layout()
            .into_iter()
            .filter(|(k, _, _)| *k == kind)
            .map(|(_, group, _)| {
                let names = files
                    .iter()
                    .filter(|f| f.kind == kind && f.group == group)
                    .map(|f| f.name.as_str())
                    .collect();
                (group, names)
            })
            .collect();
        if groups.is_empty() {
            continue;
        }
        let mut section = String::new();
        let _ = writeln!(section, "  \"{key}\": {{");
        let lines: Vec<String> = groups
            .iter()
            .map(|(group, names)| {
                let list: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
                format!("    \"{}\": [{}]", group.name(), list.join(", "))
            })
            .collect();
        let _ = writeln!(section, "{}", lines.join(",\n"));
        section.push_str("  }");
        written.push(section);
    }
    let _ = writeln!(s, "{}", written.join(",\n"));
    s.push_str("}\n");
    s
}

const LICENSE_TEXT: &str = "\
# Bundled sound packs

The packs in this folder (`blue`, `brown`, `red` and `typewriter`) are
synthesised entirely from code by the generator in
`src-tauri/examples/make_packs/`. They contain no recordings and no audio
from any other source.

To the extent possible under law, the ClickClak contributors
have waived all copyright and related or neighbouring rights to these sound
files, under the Creative Commons CC0 1.0 Universal public domain
dedication: https://creativecommons.org/publicdomain/zero/1.0/

To regenerate them after changing the generator:

    npm run make-packs
";
