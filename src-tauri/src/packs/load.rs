//! Loads a pack folder: `pack.json`, then every file it names, all or
//! nothing.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::manifest::{self, Manifest};
use super::paths::{self, PathError};
use super::{Pack, PackError, PackErrorKind as Kind, PackInfo};
use crate::audio::{self, Sample, SoundSet};
use crate::keys::KeyGroup;

/// Largest `pack.json` read.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// Largest audio file read.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Longest single sound.
pub const MAX_FILE_LENGTH: Duration = Duration::from_secs(3);
/// Most different audio files in one pack.
pub const MAX_FILES: usize = 100;
/// Most audio in one pack, all files together.
pub const MAX_TOTAL_LENGTH: Duration = Duration::from_secs(60);

/// One file name from `pack.json`, where it was found, and where it is.
struct Entry {
    /// Which list it goes in: down (0) or up (1), and the group.
    list: usize,
    group: KeyGroup,
    field: String,
    name: String,
    path: PathBuf,
}

/// The pack's id: its folder's name, or the path given if it has none.
fn id_of(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}

/// Reads and checks the `pack.json` in `dir`, as `load` does, and opens no
/// audio file. For listing packs.
pub fn read_info(dir: &Path) -> Result<PackInfo, PackError> {
    let id = id_of(dir);
    if !dir.is_dir() {
        return Err(PackError {
            pack: id,
            field: None,
            file: None,
            kind: Kind::FolderNotFound,
        });
    }
    match read_manifest(dir) {
        Ok(manifest) => Ok(PackInfo {
            id,
            name: manifest.name,
            author: manifest.author,
            license: manifest.license,
        }),
        Err((field, kind)) => Err(PackError {
            pack: id,
            field,
            file: None,
            kind,
        }),
    }
}

/// Loads the pack in `dir`. Every file is decoded into memory before this
/// returns; if anything is wrong, the whole pack is rejected.
pub fn load(dir: &Path) -> Result<Pack, PackError> {
    let id = id_of(dir);
    let fail = |field: Option<String>, file: Option<String>, kind: Kind| PackError {
        pack: id.clone(),
        field,
        file,
        kind,
    };

    let root = dir.canonicalize().map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => fail(None, None, Kind::FolderNotFound),
        _ => fail(None, None, Kind::Unreadable(error.to_string())),
    })?;
    if !root.is_dir() {
        return Err(fail(None, None, Kind::FolderNotFound));
    }

    let manifest = read_manifest(&root).map_err(|(field, kind)| fail(field, None, kind))?;

    // Every file name must resolve inside the folder before anything is read.
    let mut entries = Vec::new();
    for (list, section, groups) in [(0, "keys", &manifest.keys), (1, "up", &manifest.up)] {
        for group in KeyGroup::ALL {
            for (i, name) in groups[group.index()].iter().enumerate() {
                let field = format!("{section}.{}[{i}]", group.name());
                let path = paths::resolve(&root, name).map_err(|error| {
                    let kind = match error {
                        PathError::BadName => Kind::BadFileName,
                        PathError::NotFound => Kind::FileNotFound,
                        PathError::Outside => Kind::OutsidePack,
                        PathError::Io(error) => Kind::Unreadable(error.to_string()),
                    };
                    fail(Some(field.clone()), Some(name.clone()), kind)
                })?;
                entries.push(Entry {
                    list,
                    group,
                    field,
                    name: name.clone(),
                    path,
                });
            }
        }
    }

    let mut distinct: Vec<&PathBuf> = entries.iter().map(|entry| &entry.path).collect();
    distinct.sort();
    distinct.dedup();
    if distinct.len() > MAX_FILES {
        return Err(fail(None, None, Kind::TooManyFiles));
    }

    // Each distinct file is decoded once and shared by every group using it.
    let mut decoded: HashMap<PathBuf, Sample> = HashMap::new();
    let mut total = Duration::ZERO;
    for entry in &entries {
        if decoded.contains_key(&entry.path) {
            continue;
        }
        let sample = read_sound(&entry.path)
            .map_err(|kind| fail(Some(entry.field.clone()), Some(entry.name.clone()), kind))?;
        total += sample.duration();
        if total > MAX_TOTAL_LENGTH {
            return Err(fail(None, None, Kind::TooMuchAudio));
        }
        decoded.insert(entry.path.clone(), sample);
    }

    let mut lists: [[Vec<Sample>; 5]; 2] = Default::default();
    for entry in &entries {
        lists[entry.list][entry.group.index()].push(decoded[&entry.path].clone());
    }
    let [down, up] = lists;
    let sounds = SoundSet::new(down, up)
        .ok_or_else(|| fail(Some("keys.default".into()), None, Kind::EmptyDefault))?;

    Ok(Pack {
        id,
        name: manifest.name,
        author: manifest.author,
        license: manifest.license,
        sounds,
    })
}

fn read_manifest(root: &Path) -> Result<Manifest, (Option<String>, Kind)> {
    let path = root.join("pack.json");
    let size = match fs::metadata(&path) {
        Ok(meta) if meta.is_file() => meta.len(),
        Ok(_) => return Err((None, Kind::ManifestNotFound)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err((None, Kind::ManifestNotFound));
        }
        Err(error) => {
            return Err((
                Some("pack.json".into()),
                Kind::Unreadable(error.to_string()),
            ));
        }
    };
    if size > MAX_MANIFEST_BYTES {
        return Err((None, Kind::ManifestTooLarge));
    }
    let text = fs::read_to_string(&path).map_err(|error| {
        (
            Some("pack.json".into()),
            Kind::Unreadable(error.to_string()),
        )
    })?;
    manifest::parse(&text).map_err(|(field, kind)| (Some(field), kind))
}

fn read_sound(path: &Path) -> Result<Sample, Kind> {
    let meta = fs::metadata(path).map_err(|error| Kind::Unreadable(error.to_string()))?;
    if !meta.is_file() {
        return Err(Kind::Unreadable("not a file".into()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(Kind::FileTooLarge);
    }
    let bytes = fs::read(path).map_err(|error| Kind::Unreadable(error.to_string()))?;
    audio::decode(bytes, MAX_FILE_LENGTH).map_err(Kind::Audio)
}
