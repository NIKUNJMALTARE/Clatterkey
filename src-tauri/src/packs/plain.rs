//! A plain folder of audio files, wrapped in a pack: which files are used,
//! which keys each one is for, and the id a folder's name gives.

use std::fs;
use std::path::Path;

use super::load::MAX_FILES;
use super::manifest::Manifest;
use super::{PackError, PackErrorKind as Kind};
use crate::keys::{KeyGroup, KeyKind};

/// The licence of a pack made from a plain folder: the app does not ask.
pub const UNSTATED_LICENCE: &str = "Licence not stated";

/// The longest id made from a folder name.
const MAX_ID_CHARS: usize = 40;
/// The longest pack name taken from a folder name.
const MAX_NAME_CHARS: usize = 60;
/// The file-name endings a plain folder's audio files have, in any case.
const EXTENSIONS: [&str; 4] = [".wav", ".ogg", ".mp3", ".flac"];
/// The file-name endings that mark a sound for a key going up.
const UP_ENDINGS: [&str; 3] = ["-up", "_up", " up"];

/// The id for a pack imported from a folder called `folder_name`:
/// lower-cased, every run of other characters turned into one `-`, at most
/// 40 characters, `pack` if nothing is left, and numbered (`-2`, `-3`, …)
/// if a bundled pack has that id.
pub fn pack_id_for(folder_name: &str, is_bundled: impl Fn(&str) -> bool) -> String {
    let mut id = String::new();
    for c in folder_name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            id.push(c);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    // Every character is ASCII, so bytes are characters.
    let id = id.trim_matches('-');
    let id = id[..id.len().min(MAX_ID_CHARS)].trim_matches('-');
    let id = if id.is_empty() { "pack" } else { id };
    if !is_bundled(id) {
        return id.to_string();
    }
    (2u32..)
        .map(|n| format!("{id}-{n}"))
        .find(|numbered| !is_bundled(numbered))
        .expect("some number is free")
}

/// Which keys a file is for, from its name without the extension: an
/// ending of `-up`, `_up` or ` up` means keys going up; then a start of
/// `space`, `enter`, `backspace` or `modifier` names the group; anything
/// else is an ordinary key. Case is ignored.
pub fn assign(stem: &str) -> (KeyKind, KeyGroup) {
    let stem = stem.to_lowercase();
    let (kind, rest) = UP_ENDINGS
        .iter()
        .find_map(|ending| stem.strip_suffix(ending))
        .map_or((KeyKind::Down, stem.as_str()), |rest| (KeyKind::Up, rest));
    let group = [
        KeyGroup::Space,
        KeyGroup::Enter,
        KeyGroup::Backspace,
        KeyGroup::Modifier,
    ]
    .into_iter()
    .find(|group| rest.starts_with(group.name()))
    .unwrap_or(KeyGroup::Default);
    (kind, group)
}

/// The name without its last extension.
fn stem_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// The manifest of the pack a plain folder makes: its own audio files,
/// each assigned by name. Subfolders, hidden files and other files are
/// ignored. No audio is opened here.
pub(super) fn manifest_for(
    root: &Path,
    id: &str,
    folder_name: &str,
) -> Result<Manifest, PackError> {
    let fail = |file: Option<String>, kind: Kind| PackError {
        pack: id.to_string(),
        field: None,
        file,
        kind,
    };
    let entries =
        fs::read_dir(root).map_err(|error| fail(None, Kind::Unreadable(error.to_string())))?;

    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| fail(None, Kind::Unreadable(error.to_string())))?;
        let raw = entry.file_name();
        let lossy = raw.to_string_lossy();
        let lower = lossy.to_lowercase();
        if lossy.starts_with('.') || !EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
            continue;
        }
        // A link to a file counts; where it leads is checked by the loader.
        if !entry.path().is_file() {
            continue;
        }
        match raw.to_str() {
            // The pack format keeps these two out of file names, so that a
            // pack works on every system. macOS shows `:` as `/` in Finder.
            Some(name) if name.contains([':', '\\']) => {
                return Err(fail(Some(name.to_string()), Kind::PlainFileName));
            }
            Some(name) => names.push(name.to_string()),
            None => return Err(fail(Some(lossy.into_owned()), Kind::NotUtf8Name)),
        }
    }
    if names.is_empty() {
        return Err(fail(None, Kind::NoAudioFiles));
    }
    // Before anything is decoded: a folder of music is refused at once.
    if names.len() > MAX_FILES {
        return Err(fail(None, Kind::TooManyFiles));
    }
    // The same order on every run and every file system.
    names.sort_by(|a, b| (a.to_lowercase(), a).cmp(&(b.to_lowercase(), b)));

    let mut keys: [Vec<String>; 5] = Default::default();
    let mut up: [Vec<String>; 5] = Default::default();
    for name in names {
        let (kind, group) = assign(stem_of(&name));
        let lists = match kind {
            KeyKind::Down => &mut keys,
            KeyKind::Up => &mut up,
        };
        lists[group.index()].push(name);
    }
    if keys[KeyGroup::Default.index()].is_empty() {
        return Err(fail(None, Kind::NoOrdinarySound));
    }

    let name: String = folder_name.trim().chars().take(MAX_NAME_CHARS).collect();
    let name = name.trim();
    Ok(Manifest {
        name: if name.is_empty() { id } else { name }.to_string(),
        author: None,
        license: UNSTATED_LICENCE.to_string(),
        keys,
        up,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stem_is_the_name_without_its_last_extension() {
        assert_eq!(stem_of("Space 2.flac"), "Space 2");
        assert_eq!(stem_of("a.b.wav"), "a.b");
        assert_eq!(stem_of("plain"), "plain");
    }
}
