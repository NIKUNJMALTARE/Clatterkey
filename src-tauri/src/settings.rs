//! The settings file, `settings.json` in the app config folder.
//!
//! It is checked by hand over a JSON value, so every problem names its
//! field. A field that is missing, of the wrong type or out of range takes
//! its default; a file that cannot be read as a JSON object gives every
//! default and is left as it is, so a user's edit is never destroyed.
//! Nothing here runs on the key path.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::{ErrorOut, write_line};

/// The pack played when the settings name none that can be loaded.
pub const DEFAULT_PACK: &str = "brown";

/// Every setting. Only `pack_id` changes what is heard so far; the others
/// are read and kept for the settings window, sound tuning and launch at
/// login.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub pack_id: String,
    /// 0.0 to 1.0.
    pub volume: f64,
    /// 0.0 to 0.3.
    pub pitch_variation: f64,
    pub key_up_sounds: bool,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            pack_id: DEFAULT_PACK.to_string(),
            volume: 0.8,
            pitch_variation: 0.0,
            key_up_sounds: true,
            launch_at_login: false,
        }
    }
}

/// Settings read from a file, with the fields that took their default
/// because they were missing, of the wrong type or out of range, in the
/// file's schema order.
#[derive(Debug)]
pub struct ReadSettings {
    pub settings: Settings,
    pub replaced: Vec<&'static str>,
}

/// Why no settings could be read from a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    NotFound,
    /// Unreadable, not valid JSON, or not a JSON object; says which.
    Invalid(String),
}

/// Reads and checks the settings file.
pub fn read(path: &Path) -> Result<ReadSettings, ReadError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        // A parent that is a regular file also means there is no file.
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Err(ReadError::NotFound);
        }
        Err(e) => return Err(ReadError::Invalid(e.to_string())),
    };
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        ReadError::Invalid(format!(
            "invalid JSON at line {} column {}",
            e.line(),
            e.column()
        ))
    })?;
    let Value::Object(map) = value else {
        return Err(ReadError::Invalid("not a JSON object".to_string()));
    };
    Ok(from_map(&map))
}

/// A bool field, or its default (recorded in `replaced`).
fn flag(
    map: &Map<String, Value>,
    name: &'static str,
    default: bool,
    replaced: &mut Vec<&'static str>,
) -> bool {
    match map.get(name) {
        Some(Value::Bool(b)) => *b,
        _ => {
            replaced.push(name);
            default
        }
    }
}

/// A number field in `0.0..=max`, or its default (recorded in `replaced`).
fn number(
    map: &Map<String, Value>,
    name: &'static str,
    max: f64,
    default: f64,
    replaced: &mut Vec<&'static str>,
) -> f64 {
    match map.get(name).and_then(Value::as_f64) {
        Some(v) if (0.0..=max).contains(&v) => v,
        _ => {
            replaced.push(name);
            default
        }
    }
}

fn from_map(map: &Map<String, Value>) -> ReadSettings {
    let d = Settings::default();
    let mut replaced = Vec::new();
    let enabled = flag(map, "enabled", d.enabled, &mut replaced);
    let pack_id = match map.get("pack_id") {
        Some(Value::String(s)) => s.clone(),
        _ => {
            replaced.push("pack_id");
            d.pack_id.clone()
        }
    };
    let volume = number(map, "volume", 1.0, d.volume, &mut replaced);
    let pitch_variation = number(
        map,
        "pitch_variation",
        0.3,
        d.pitch_variation,
        &mut replaced,
    );
    let key_up_sounds = flag(map, "key_up_sounds", d.key_up_sounds, &mut replaced);
    let launch_at_login = flag(map, "launch_at_login", d.launch_at_login, &mut replaced);
    ReadSettings {
        settings: Settings {
            enabled,
            pack_id,
            volume,
            pitch_variation,
            key_up_sounds,
            launch_at_login,
        },
        replaced,
    }
}

/// The file's text: the six fields, in schema order.
fn to_json(settings: &Settings) -> String {
    let field = |v: Value| v.to_string();
    format!(
        "{{\n  \"enabled\": {},\n  \"pack_id\": {},\n  \"volume\": {},\n  \"pitch_variation\": {},\n  \"key_up_sounds\": {},\n  \"launch_at_login\": {}\n}}\n",
        field(Value::Bool(settings.enabled)),
        field(Value::String(settings.pack_id.clone())),
        field(json_number(settings.volume)),
        field(json_number(settings.pitch_variation)),
        field(Value::Bool(settings.key_up_sounds)),
        field(Value::Bool(settings.launch_at_login)),
    )
}

/// A JSON number; a value JSON cannot hold (NaN, infinity) is written as 0.
fn json_number(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::from(0), Value::Number)
}

/// Saves the settings atomically: the text goes to a temporary file beside
/// the target, which is then renamed over it, so a crash never leaves a
/// half-written file. Creates the folder if needed.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_path(path);
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(to_json(settings).as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// The settings to start with. A missing file is created with every default.
/// Any problem is written to `err` as one `settings:` line and the defaults
/// stand in; an existing file is never rewritten here.
pub fn load_or_create(path: &Path, err: &ErrorOut) -> Settings {
    match read(path) {
        Ok(read) => {
            if !read.replaced.is_empty() {
                write_line(
                    err,
                    &format!(
                        "settings: invalid {}; using defaults for those",
                        read.replaced.join(", ")
                    ),
                );
            }
            read.settings
        }
        Err(ReadError::NotFound) => {
            let settings = Settings::default();
            if let Err(e) = save(path, &settings) {
                write_line(
                    err,
                    &format!("settings: could not save {}: {e}", path.display()),
                );
            }
            settings
        }
        Err(ReadError::Invalid(why)) => {
            write_line(
                err,
                &format!("settings: {}: {why}; using defaults", path.display()),
            );
            Settings::default()
        }
    }
}
