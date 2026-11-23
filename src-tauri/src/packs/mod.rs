//! Sound packs: a folder with a `pack.json` and the audio files it names.
//!
//! `load` reads and checks a pack and decodes all its files into memory, or
//! returns one error saying what is wrong and where. It never returns part
//! of a pack. Decoding is done by the `audio` module.

mod load;
mod manifest;
mod paths;

use std::fmt;

use crate::audio::{DecodeError, SoundSet};

pub use load::{
    MAX_FILE_BYTES, MAX_FILE_LENGTH, MAX_FILES, MAX_MANIFEST_BYTES, MAX_TOTAL_LENGTH, load,
};

/// A loaded pack, every sound in memory.
#[derive(Debug)]
pub struct Pack {
    /// The folder's name.
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub license: String,
    pub sounds: SoundSet,
}

/// Why a pack was rejected. Shown as one line:
/// `pack "blue": keys.space[0] "down/space.wav": file not found`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackError {
    /// The pack's id, or the path given if the folder has no name.
    pub pack: String,
    /// The field in `pack.json` it is about, such as `keys.space[0]`.
    pub field: Option<String>,
    /// The file name as written in `pack.json`.
    pub file: Option<String>,
    pub kind: PackErrorKind,
}

/// What is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackErrorKind {
    FolderNotFound,
    ManifestNotFound,
    ManifestTooLarge,
    /// Could not read a file or folder.
    Unreadable(String),
    BadJson {
        line: usize,
        column: usize,
    },
    Missing,
    NotAnObject,
    NotText,
    NotAList,
    NotAFileName,
    UnknownGroup,
    EmptyDefault,
    /// Absolute, `..`, `\`, `:` or `.` in a file name.
    BadFileName,
    /// Through a link, the file is outside the pack folder.
    OutsidePack,
    FileNotFound,
    FileTooLarge,
    Audio(DecodeError),
    TooManyFiles,
    TooMuchAudio,
}

impl fmt::Display for PackErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PackErrorKind::*;
        match self {
            FolderNotFound => f.write_str("folder not found"),
            ManifestNotFound => f.write_str("pack.json not found"),
            ManifestTooLarge => write!(
                f,
                "pack.json is larger than {} KiB",
                MAX_MANIFEST_BYTES / 1024
            ),
            Unreadable(reason) => write!(f, "could not be read: {reason}"),
            BadJson { line, column } => {
                write!(f, "not valid JSON (line {line}, column {column})")
            }
            Missing => f.write_str("missing"),
            NotAnObject => f.write_str("must be an object"),
            NotText => f.write_str("must be a non-empty string"),
            NotAList => f.write_str("must be a list of file names"),
            NotAFileName => f.write_str("must be a file name"),
            UnknownGroup => {
                f.write_str("unknown group (use default, space, enter, backspace or modifier)")
            }
            EmptyDefault => f.write_str("must list at least one file"),
            BadFileName => f.write_str(
                "file names must be relative to the pack folder, use /, and not use . or ..",
            ),
            OutsidePack => f.write_str("is outside the pack folder"),
            FileNotFound => f.write_str("file not found"),
            FileTooLarge => write!(f, "larger than {} MB", MAX_FILE_BYTES / (1024 * 1024)),
            Audio(DecodeError::TooLong) => {
                write!(f, "longer than {} seconds", MAX_FILE_LENGTH.as_secs())
            }
            Audio(error) => error.fmt(f),
            TooManyFiles => write!(f, "uses more than {MAX_FILES} different audio files"),
            TooMuchAudio => write!(
                f,
                "has more than {} seconds of audio in total",
                MAX_TOTAL_LENGTH.as_secs()
            ),
        }
    }
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pack {:?}: ", self.pack)?;
        match (&self.field, &self.file) {
            (Some(field), Some(file)) => write!(f, "{field} {file:?}: ")?,
            (Some(field), None) => write!(f, "{field}: ")?,
            (None, Some(file)) => write!(f, "{file:?}: ")?,
            (None, None) => {}
        }
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for PackError {}
