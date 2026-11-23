//! Where packs live: the bundled folder that ships with the app, and the
//! folder in the app's data that imported packs are copied into. An id is
//! looked for among the bundled packs first, so an imported folder can never
//! stand in for a bundled pack.

use std::fs;
use std::path::{Path, PathBuf};

use super::{PackInfo, read_info};

/// A folder being filled by an import; never a valid id, so never listed.
pub(super) const IMPORTING: &str = ".import-";
/// A pack moved aside while its replacement is put in place.
pub(super) const OLD: &str = ".old-";
/// A removed pack on its way to being deleted.
pub(super) const REMOVED: &str = ".removed-";

/// The two pack folders. Either may be unknown.
#[derive(Debug, Clone, Default)]
pub struct Library {
    bundled: Option<PathBuf>,
    imported: Option<PathBuf>,
}

/// Where a pack is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub dir: PathBuf,
    /// Whether it is in the imported folder.
    pub imported: bool,
}

/// A pack as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub info: PackInfo,
    pub imported: bool,
}

/// Whether `id` can name a pack: non-empty, and only ASCII letters, digits,
/// `-` and `_`. This rules out path separators, `.`, `..`, `:` and drive
/// prefixes on every OS before any path is built from it.
pub fn is_pack_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether `id` is exactly the name of a folder in `packs`. Matching on
/// the listed names, not by opening the path, keeps ids case-sensitive on
/// macOS and Windows too, so `"Brown"` is unknown everywhere.
fn names_a_pack(packs: &Path, id: &str) -> bool {
    fs::read_dir(packs).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| e.file_name() == id && e.path().is_dir())
    })
}

impl Library {
    pub fn new(bundled: Option<PathBuf>, imported: Option<PathBuf>) -> Self {
        Self { bundled, imported }
    }

    /// Only the bundled folder: nothing can be imported.
    pub fn bundled_only(bundled: PathBuf) -> Self {
        Self::new(Some(bundled), None)
    }

    /// No folders at all.
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether the bundled packs folder is known.
    pub(crate) fn has_bundled_folder(&self) -> bool {
        self.bundled.is_some()
    }

    /// Where the bundled default pack is, or would be.
    pub(crate) fn default_dir(&self) -> PathBuf {
        self.bundled
            .as_deref()
            .unwrap_or(Path::new(""))
            .join(crate::settings::DEFAULT_PACK)
    }

    /// The folder imported packs are kept in, which may not exist yet.
    pub fn imported_dir(&self) -> Option<&Path> {
        self.imported.as_deref()
    }

    /// Whether `id` names a bundled pack. The id is checked before any path
    /// is built from it.
    pub fn is_bundled(&self, id: &str) -> bool {
        is_pack_id(id)
            && self
                .bundled
                .as_deref()
                .is_some_and(|dir| names_a_pack(dir, id))
    }

    /// The folder of the pack `id`: a bundled one first, then an imported
    /// one. `None` for anything that is not a valid id.
    pub fn find(&self, id: &str) -> Option<Found> {
        if !is_pack_id(id) {
            return None;
        }
        let folders = [(&self.bundled, false), (&self.imported, true)];
        folders.into_iter().find_map(|(folder, imported)| {
            let folder = folder.as_deref()?;
            names_a_pack(folder, id).then(|| Found {
                dir: folder.join(id),
                imported,
            })
        })
    }

    /// Every pack whose folder name is a valid id and whose `pack.json` can
    /// be read, sorted by name. An imported folder with a bundled pack's id
    /// is left out. No audio is opened.
    pub fn list(&self) -> Vec<Listed> {
        let mut found: Vec<Listed> = Vec::new();
        for (folder, imported) in [(&self.bundled, false), (&self.imported, true)] {
            let Some(folder) = folder.as_deref() else {
                continue;
            };
            let Ok(entries) = fs::read_dir(folder) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(id) = name.to_str().filter(|id| is_pack_id(id)) else {
                    continue;
                };
                // Even a bundled pack that cannot be listed keeps its id.
                if imported && self.is_bundled(id) {
                    continue;
                }
                if let Ok(info) = read_info(&folder.join(id)) {
                    found.push(Listed { info, imported });
                }
            }
        }
        found.sort_by(|a, b| {
            let (a, b) = (&a.info, &b.info);
            (a.name.to_lowercase(), &a.name, &a.id).cmp(&(b.name.to_lowercase(), &b.name, &b.id))
        });
        found
    }

    /// Clears what an import or a removal left behind if the app stopped
    /// part-way: half-made copies and removed packs are deleted; a pack
    /// that was moved aside and whose replacement never arrived is put
    /// back. Errors are ignored: this is tidying.
    pub fn clean_temporary(&self) {
        let Some(folder) = self.imported.as_deref() else {
            return;
        };
        let Ok(entries) = fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let path = entry.path();
            if name.starts_with(IMPORTING) || name.starts_with(REMOVED) {
                let _ = remove(&path);
            } else if let Some(rest) = name.strip_prefix(OLD) {
                // `.old-<id>-<counter>`: the id is all but the last part.
                let id = rest.rsplit_once('-').map_or(rest, |(id, _)| id);
                let home = folder.join(id);
                if is_pack_id(id) && !home.exists() {
                    let _ = fs::rename(&path, &home);
                } else {
                    let _ = remove(&path);
                }
            }
        }
    }
}

/// Deletes a folder and all in it, or a stray file.
pub(super) fn remove(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}
