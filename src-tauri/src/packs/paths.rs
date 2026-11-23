//! File names in `pack.json`, and keeping them inside the pack folder.

use std::io;
use std::path::{Path, PathBuf};

/// Why a file name was refused.
#[derive(Debug)]
pub(super) enum PathError {
    /// Empty, absolute, uses `\` or `:`, or has a `.` or `..` part. Found
    /// without touching the disk.
    BadName,
    /// The file does not exist.
    NotFound,
    /// It exists, and through a link it is outside the pack folder.
    Outside,
    /// The disk could not say where it is.
    Io(io::Error),
}

/// The real location of `name` inside the pack folder `root`, which must
/// already be canonical. Names use `/` on every OS. Nothing outside `root`
/// is ever returned, so nothing outside it is read.
pub(super) fn resolve(root: &Path, name: &str) -> Result<PathBuf, PathError> {
    let bad = name.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.split('/').any(|part| part == ".." || part == ".");
    if bad {
        return Err(PathError::BadName);
    }

    let mut path = root.to_path_buf();
    for part in name.split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    let real = path.canonicalize().map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => PathError::NotFound,
        _ => PathError::Io(error),
    })?;
    if real.starts_with(root) {
        Ok(real)
    } else {
        Err(PathError::Outside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_names_are_refused_before_the_disk_is_touched() {
        // The root does not exist, so only the name rule can answer.
        let root = Path::new("/no/such/pack/folder");
        for name in [
            "",
            "/etc/x.wav",
            "../outside.wav",
            "down/../../x.wav",
            "./a.wav",
            "C:\\x.wav",
            "C:x.wav",
            "down\\a.wav",
            "a.wav:stream",
        ] {
            assert!(
                matches!(resolve(root, name), Err(PathError::BadName)),
                "{name:?}"
            );
        }
    }

    #[test]
    fn a_good_name_that_is_missing_is_not_found() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        assert!(matches!(
            resolve(&root, "mks-no-such-file-7c1e.wav"),
            Err(PathError::NotFound)
        ));
    }
}
