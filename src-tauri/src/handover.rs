//! The hand-over from the app's earlier name, Mechanical Key Sounds.
//!
//! To the system the renamed app is a new one: it has a new folder and a
//! new login registration. At start-up, before the settings are read, what
//! the old app left is carried over: its settings file and imported packs
//! are copied (the old folder is only ever read), and its login
//! registration is removed so that the two never start side by side.
//!
//! No Tauri in here, so it is tested with temporary folders and a fake
//! registration. The old identifier and the old name live in this module
//! alone.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::autostart::LoginItem;
use crate::packs::rename as rename_patiently;
use crate::{ErrorOut, write_line};

/// The old app's identifier: the name of its settings folder.
pub const OLD_IDENTIFIER: &str = "com.mechanicalkeysounds.app";
/// The old app's name: the name of its login registration.
pub const OLD_APP_NAME: &str = "Mechanical Key Sounds";

const SETTINGS: &str = "settings.json";
const PACKS: &str = "packs";
/// Where the copy is put together, inside the new folder, before it is
/// renamed into place.
const TEMPORARY: &str = ".handover-tmp";

/// What `copy_files` did.
pub enum Copied {
    /// There was nothing to copy, or the new folder is already in use.
    Nothing,
    /// The old settings, the old packs, or both are now in the new folder.
    Done,
    /// The copy failed and left nothing behind; one line was written.
    Failed,
}

/// The old app's folder: beside the new one, named by the old identifier.
pub fn old_folder(new_folder: &Path) -> Option<PathBuf> {
    new_folder
        .parent()
        .map(|parent| parent.join(OLD_IDENTIFIER))
}

/// Copies the old settings file and the old packs folder into the new
/// folder, if the new folder has neither yet.
///
/// Once the new folder holds a settings file or a packs folder the old one
/// is never looked at again, so nothing saved under the new name is ever
/// overwritten. The copy is put together in a temporary folder and renamed
/// into place, the settings file last, so a settings file in the new
/// folder always means a complete copy. On a failure nothing of the
/// attempt is left and one `migrate:` line is written.
pub fn copy_files(old: &Path, new: &Path, err: &ErrorOut) -> Copied {
    if new.join(SETTINGS).exists() || new.join(PACKS).exists() {
        return Copied::Nothing;
    }
    if !old.is_dir() {
        return Copied::Nothing;
    }
    let (settings, packs) = (old.join(SETTINGS), old.join(PACKS));
    let (has_settings, has_packs) = (settings.is_file(), packs.is_dir());
    if !has_settings && !has_packs {
        return Copied::Nothing;
    }

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
        Ok(()) => Copied::Done,
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

/// Takes the old app's login registration out, and puts the new app's in
/// its place if the settings just copied have Launch at login on and the
/// old registration was in force.
///
/// The old one is removed at every start, whatever was copied: the old
/// app, if still installed, may have been opened and registered again,
/// and both would then start at login and every key would sound twice.
/// A build that does not register (`registers` false, a development
/// build) touches neither. A failure writes one `migrate:` line and the
/// start goes on; the usual start-up comparison then decides the setting.
pub fn login(
    copied: Copied,
    setting_on: bool,
    registers: bool,
    old: &dyn LoginItem,
    new: &dyn LoginItem,
    err: &ErrorOut,
) {
    if !registers {
        return;
    }
    let was_in_force = match old.is_enabled() {
        Ok(in_force) => in_force,
        Err(error) => {
            write_line(
                err,
                &format!("migrate: could not read the old login item: {error}"),
            );
            false
        }
    };
    if let Err(error) = old.disable() {
        write_line(
            err,
            &format!("migrate: could not remove the old login item: {error}"),
        );
    }
    if matches!(copied, Copied::Done)
        && setting_on
        && was_in_force
        && let Err(error) = new.enable()
    {
        write_line(
            err,
            &format!("migrate: could not make the login item: {error}"),
        );
    }
}
