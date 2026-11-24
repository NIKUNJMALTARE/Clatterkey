//! The hand-over from the app's earlier names, ClickClak and Mechanical
//! Key Sounds.
//!
//! To the system the renamed app is a new one: it has a new folder and a
//! new login registration. At start-up, before the settings are read, what
//! an earlier app left is carried over: the settings file and imported
//! packs of the newest earlier name that has any are copied (the earlier
//! folders are only ever read), and the login registrations of all the
//! earlier names are removed so that no two ever start side by side.
//!
//! No Tauri in here, so it is tested with temporary folders and a fake
//! registration. The earlier identifiers and names live in this module
//! alone.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::autostart::LoginItem;
use crate::packs::rename as rename_patiently;
use crate::{ErrorOut, write_line};

/// A name the app had before the present one.
pub struct Earlier {
    /// The identifier: the name of its settings folder.
    pub identifier: &'static str,
    /// The app name: the name of its login registration.
    pub app_name: &'static str,
}

/// The earlier names, newest first. The folders and the registrations
/// given to `copy_files` and `login` are built from this list, in this
/// order, so that a position means the same name in both.
pub const EARLIER: [Earlier; 2] = [
    Earlier {
        identifier: "com.clickclak.app",
        app_name: "ClickClak",
    },
    Earlier {
        identifier: "com.mechanicalkeysounds.app",
        app_name: "Mechanical Key Sounds",
    },
];

const SETTINGS: &str = "settings.json";
const PACKS: &str = "packs";
/// Where the copy is put together, inside the new folder, before it is
/// renamed into place.
const TEMPORARY: &str = ".handover-tmp";

/// What `copy_files` did.
pub enum Copied {
    /// There was nothing to copy, or the new folder is already in use.
    Nothing,
    /// The settings, the packs, or both of the earlier folder at this
    /// position in the list given are now in the new folder.
    Done(usize),
    /// The copy failed and left nothing behind; one line was written.
    Failed,
}

/// The earlier apps' folders: beside the new one, named by the earlier
/// identifiers, in the order of `EARLIER`.
pub fn earlier_folders(new_folder: &Path) -> Vec<PathBuf> {
    match new_folder.parent() {
        Some(parent) => EARLIER
            .iter()
            .map(|earlier| parent.join(earlier.identifier))
            .collect(),
        None => Vec::new(),
    }
}

/// Copies the settings file and the packs folder of one earlier folder
/// into the new folder, if the new folder has neither yet.
///
/// The source is the first folder in the list that holds a settings file
/// or a packs folder; the others are not read and nothing is merged, since
/// a newer name's folder already holds what it carried over from an older
/// one. Once the new folder holds a settings file or a packs folder no
/// earlier one is looked at again, so nothing saved under the new name is
/// ever overwritten. The copy is put together in a temporary folder and
/// renamed into place, the settings file last, so a settings file in the
/// new folder always means a complete copy. On a failure nothing of the
/// attempt is left, one `migrate:` line is written, and no later folder in
/// the list is tried in its place.
pub fn copy_files(earlier: &[PathBuf], new: &Path, err: &ErrorOut) -> Copied {
    if new.join(SETTINGS).exists() || new.join(PACKS).exists() {
        return Copied::Nothing;
    }
    let source = earlier.iter().enumerate().find(|(_, folder)| {
        folder.is_dir() && (folder.join(SETTINGS).is_file() || folder.join(PACKS).is_dir())
    });
    let Some((position, old)) = source else {
        return Copied::Nothing;
    };
    let (settings, packs) = (old.join(SETTINGS), old.join(PACKS));
    let (has_settings, has_packs) = (settings.is_file(), packs.is_dir());

    let made_new = !new.exists();
    let temporary = new.join(TEMPORARY);
    let result = (|| -> Result<(), (PathBuf, io::Error)> {
        let at = |path: &Path| {
            let path = path.to_path_buf();
            move |error| (path, error)
        };
        // What a stopped app left of an earlier attempt.
        if temporary.exists() {
            fs::remove_dir_all(&temporary).map_err(at(&temporary))?;
        }
        fs::create_dir_all(&temporary).map_err(at(&temporary))?;
        if has_packs {
            copy_folder(&packs, &temporary.join(PACKS))?;
        }
        if has_settings {
            fs::copy(&settings, temporary.join(SETTINGS)).map_err(at(&settings))?;
        }
        if has_packs {
            // The files are new, and on Windows a scanner may still hold
            // one open: the rename waits for it, as an import's does.
            rename_patiently(&temporary.join(PACKS), &new.join(PACKS))
                .map_err(at(&new.join(PACKS)))?;
        }
        if has_settings {
            rename_patiently(&temporary.join(SETTINGS), &new.join(SETTINGS))
                .map_err(at(&new.join(SETTINGS)))?;
        }
        Ok(())
    })();

    // The temporary folder goes in either case; a failure to remove it is
    // not a failed copy, and the next start-up clears it.
    let _ = fs::remove_dir_all(&temporary);
    match result {
        Ok(()) => Copied::Done(position),
        Err((path, error)) => {
            // The settings file is renamed last, so only the packs can
            // have reached their place.
            let _ = fs::remove_dir_all(new.join(PACKS));
            if made_new {
                // Only if it is empty, which it is unless something else
                // wrote there meanwhile.
                let _ = fs::remove_dir(new);
            }
            write_line(
                err,
                &format!(
                    "migrate: could not copy the settings and packs from {}: {}: {error}; starting with the defaults",
                    old.display(),
                    path.display()
                ),
            );
            Copied::Failed
        }
    }
}

/// Copies a folder with the folders and regular files in it. Anything
/// else (a link, a device) is left out.
fn copy_folder(from: &Path, to: &Path) -> Result<(), (PathBuf, io::Error)> {
    let at = |path: &Path| {
        let path = path.to_path_buf();
        move |error| (path, error)
    };
    fs::create_dir_all(to).map_err(at(to))?;
    for entry in fs::read_dir(from).map_err(at(from))? {
        let entry = entry.map_err(at(from))?;
        let source = entry.path();
        let kind = entry.file_type().map_err(at(&source))?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_folder(&source, &target)?;
        } else if kind.is_file() {
            fs::copy(&source, &target).map_err(at(&source))?;
        }
    }
    Ok(())
}

/// Takes the earlier apps' login registrations out, and puts the new
/// app's in their place if the settings just copied have Launch at login
/// on and the registration of the name they were copied from was in force.
///
/// The registrations are in the order of the folders given to
/// `copy_files`. All of them are removed at every start, whatever was
/// copied: an earlier app, if still installed, may have been opened and
/// registered again, and both would then start at login and every key
/// would sound twice. A build that does not register (`registers` false, a
/// development build) touches none. A failure writes one `migrate:` line
/// and the rest goes on; the usual start-up comparison then decides the
/// setting.
pub fn login(
    copied: Copied,
    setting_on: bool,
    registers: bool,
    earlier: &[&dyn LoginItem],
    new: &dyn LoginItem,
    err: &ErrorOut,
) {
    if !registers {
        return;
    }
    let mut source_was_in_force = false;
    for (position, item) in earlier.iter().enumerate() {
        let was_in_force = match item.is_enabled() {
            Ok(in_force) => in_force,
            Err(error) => {
                write_line(
                    err,
                    &format!("migrate: could not read an earlier login item: {error}"),
                );
                false
            }
        };
        if matches!(copied, Copied::Done(source) if source == position) {
            source_was_in_force = was_in_force;
        }
        if let Err(error) = item.disable() {
            write_line(
                err,
                &format!("migrate: could not remove an earlier login item: {error}"),
            );
        }
    }
    if setting_on
        && source_was_in_force
        && let Err(error) = new.enable()
    {
        write_line(
            err,
            &format!("migrate: could not make the login item: {error}"),
        );
    }
}
