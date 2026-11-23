//! Importing a pack: a folder the user chose is checked whole where it is,
//! copied into the app's own packs folder, and put in place in one step.
//! The user's folder is only ever read.

use std::fmt;
use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::library::{IMPORTING, Library, OLD, REMOVED, remove};
use super::load::{self, Loaded, Style};
use super::plain::{manifest_for, pack_id_for};
use super::{PackError, PackErrorKind as Kind, PackInfo};
use crate::audio::SoundSet;

/// An imported pack, in place and ready to play.
#[derive(Debug)]
pub struct Imported {
    pub info: PackInfo,
    /// Whether an earlier imported pack with this id was replaced.
    pub replaced: bool,
    /// The pack's sounds, decoded while it was checked.
    pub sounds: SoundSet,
}

/// Why a folder was not imported. Nothing has changed when one is returned.
#[derive(Debug)]
pub enum ImportError {
    /// The folder is not a usable pack.
    Pack(PackError),
    /// It is usable, and could not be copied or put in place.
    Copy(String),
    /// There is nowhere to copy it to.
    NoPacksFolder,
    /// The folder chosen is inside the app's own packs folder.
    AlreadyInside,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Pack(error) => f.write_str(&error.reason()),
            ImportError::Copy(reason) => write!(f, "could not be copied: {reason}"),
            ImportError::NoPacksFolder => f.write_str("the app data folder was not found"),
            ImportError::AlreadyInside => {
                f.write_str("this folder is the app's own copy of the pack")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Makes the names of temporary folders differ within this run.
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn next() -> u64 {
    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

/// A name for a folder that is being moved out of the way.
fn aside(prefix: &str, id: &str) -> String {
    format!("{prefix}{id}-{}", next())
}

/// The name a removed pack's folder is given until it is deleted.
pub(crate) fn removed_name(id: &str) -> String {
    aside(REMOVED, id)
}

/// Imports the folder `source` into the library's imported folder. A folder
/// with a `pack.json` is a pack; any other is a plain folder of audio files.
pub fn import(source: &Path, library: &Library) -> Result<Imported, ImportError> {
    import_with(source, library, &|from, to| fs::copy(from, to).map(|_| ()))
}

/// As `import`, copying each file with `copy`.
pub fn import_with(
    source: &Path,
    library: &Library,
    copy: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<Imported, ImportError> {
    let root = load::open_folder(source).map_err(ImportError::Pack)?;
    let packs = library.imported_dir();
    // Replacing the app's copy with itself would delete it.
    if let Some(packs) = packs
        && let Ok(packs) = packs.canonicalize()
        && root.starts_with(&packs)
    {
        return Err(ImportError::AlreadyInside);
    }

    let folder_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let id = pack_id_for(&folder_name, |id| library.is_bundled(id));

    // The whole pack is checked, and decoded, before anything is written.
    let (loaded, manifest_text) = check(&root, &id, &folder_name)?;

    let packs = packs.ok_or(ImportError::NoPacksFolder)?;
    let copy_error = |error: io::Error| ImportError::Copy(error.to_string());
    fs::create_dir_all(packs).map_err(copy_error)?;
    let temporary = packs.join(format!("{IMPORTING}{id}-{}-{}", std::process::id(), next()));
    if let Err(error) = fill(&temporary, &loaded, &manifest_text, copy) {
        let _ = remove(&temporary);
        return Err(copy_error(error));
    }

    // The copy is what will be read at every later start, so it is the copy
    // that is loaded and played: if the user's folder changed between the
    // check and the copy, the two cannot disagree.
    let stored = match catch_unwind(AssertUnwindSafe(|| load::load_folder(&temporary))) {
        Ok(Ok(stored)) => stored,
        _ => {
            let _ = remove(&temporary);
            return Err(ImportError::Copy(
                "the folder changed while it was being copied".into(),
            ));
        }
    };

    let home = packs.join(&id);
    let replaced = match put_in_place(&temporary, &home, packs, &id) {
        Ok(replaced) => replaced,
        Err(error) => {
            let _ = remove(&temporary);
            return Err(copy_error(error));
        }
    };

    let Loaded {
        pack, has_key_up, ..
    } = stored;
    Ok(Imported {
        info: PackInfo {
            id,
            name: pack.name,
            author: pack.author,
            license: pack.license,
            has_key_up,
        },
        replaced,
        sounds: pack.sounds,
    })
}

/// Loads the source as the pack it is, or as the pack a plain folder makes,
/// and gives the `pack.json` its copy will have: the source's own bytes, or
/// the one the app writes for a plain folder. A decoder that panics on a
/// malformed file is an invalid pack.
fn check(root: &Path, id: &str, folder_name: &str) -> Result<(Loaded, Vec<u8>), ImportError> {
    let loading = || -> Result<(Loaded, Vec<u8>), PackError> {
        if root.join("pack.json").is_file() {
            let loaded = load::load_folder(root)?;
            let text = fs::read(root.join("pack.json")).map_err(|error| PackError {
                pack: id.to_string(),
                field: Some("pack.json".into()),
                file: None,
                kind: Kind::Unreadable(error.to_string()),
            })?;
            Ok((loaded, text))
        } else {
            let manifest = manifest_for(root, id, folder_name)?;
            let text = manifest.to_json().into_bytes();
            let loaded = load::load_from(
                root.to_path_buf(),
                id.to_string(),
                manifest,
                Style::PlainFolder,
            )?;
            Ok((loaded, text))
        }
    };
    match catch_unwind(AssertUnwindSafe(loading)) {
        Ok(result) => result.map_err(ImportError::Pack),
        Err(_) => Err(ImportError::Pack(PackError {
            pack: id.to_string(),
            field: None,
            file: None,
            kind: Kind::Unreadable("an audio file could not be decoded".into()),
        })),
    }
}

/// Copies the pack's files and its `pack.json` into the new folder `to`.
fn fill(
    to: &Path,
    loaded: &Loaded,
    manifest: &[u8],
    copy: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir(to)?;
    for (name, real) in &loaded.files {
        // Names were checked by the loader: relative, `/` between parts, no
        // `.` or `..`. Empty parts are skipped, as the loader skips them.
        let mut dest = to.to_path_buf();
        dest.extend(name.split('/').filter(|part| !part.is_empty()));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if !dest.exists() {
            copy(real, &dest)?;
        }
    }
    fs::write(to.join("pack.json"), manifest)
}

/// How many times a refused rename is tried, and the first wait between
/// tries; each wait is longer than the last (about 0.75 s in all).
const RENAME_TRIES: u32 = 6;
const RENAME_WAIT: Duration = Duration::from_millis(50);

/// Renames a folder. On Windows a virus scanner or the search indexer may
/// hold a newly written file open for a moment, and the folder cannot be
/// renamed until it lets go; a refusal is tried again a few times.
pub(crate) fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut tried = 1;
    loop {
        match fs::rename(from, to) {
            Err(error)
                if error.kind() == io::ErrorKind::PermissionDenied && tried < RENAME_TRIES =>
            {
                std::thread::sleep(RENAME_WAIT * tried);
                tried += 1;
            }
            result => return result,
        }
    }
}

/// Makes the finished copy `temporary` the pack at `home`. An earlier pack
/// there is moved aside first, and put back if the new one cannot take its
/// place. Returns whether there was one.
fn put_in_place(temporary: &Path, home: &Path, packs: &Path, id: &str) -> io::Result<bool> {
    let old: Option<PathBuf> = if home.exists() {
        let old = packs.join(aside(OLD, id));
        rename(home, &old)?;
        Some(old)
    } else {
        None
    };
    if let Err(error) = rename(temporary, home) {
        if let Some(old) = &old {
            let _ = rename(old, home);
        }
        return Err(error);
    }
    if let Some(old) = &old {
        // If this fails the old copy is cleared at the next start-up.
        let _ = remove(old);
    }
    Ok(old.is_some())
}
