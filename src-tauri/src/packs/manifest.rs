//! Reads and checks `pack.json`. Checked by hand over a JSON value rather
//! than derived, so every error can name the field it is about.

use serde_json::{Map, Value};

use super::PackErrorKind as Kind;
use crate::keys::KeyGroup;

/// What `pack.json` says, checked.
#[derive(Debug)]
pub(super) struct Manifest {
    pub name: String,
    pub author: Option<String>,
    pub license: String,
    /// File names per group, indexed by `KeyGroup::index`.
    pub keys: [Vec<String>; 5],
    pub up: [Vec<String>; 5],
}

/// A problem with one field: the field's path (`keys.space[0]`) and what is
/// wrong with it.
pub(super) type FieldError = (String, Kind);

/// Parses and checks the text of `pack.json`. Unknown top-level fields are
/// ignored; an unknown group name is an error.
pub(super) fn parse(text: &str) -> Result<Manifest, FieldError> {
    // Some Windows editors start the file with a byte-order mark.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: Value = serde_json::from_str(text).map_err(|error| {
        (
            "pack.json".to_string(),
            Kind::BadJson {
                line: error.line(),
                column: error.column(),
            },
        )
    })?;
    let Value::Object(top) = value else {
        return Err(("pack.json".into(), Kind::NotAnObject));
    };

    let name = required_text(&top, "name")?;
    let license = required_text(&top, "license")?;
    let author = match top.get("author") {
        None | Some(Value::Null) => None,
        Some(Value::String(author)) => Some(author.clone()),
        Some(_) => return Err(("author".into(), Kind::NotText)),
    };

    let keys = match top.get("keys") {
        None | Some(Value::Null) => return Err(("keys".into(), Kind::Missing)),
        Some(value) => groups(value, "keys")?,
    };
    if keys[KeyGroup::Default.index()].is_empty() {
        let kind = match top.get("keys").and_then(|keys| keys.get("default")) {
            None | Some(Value::Null) => Kind::Missing,
            Some(_) => Kind::EmptyDefault,
        };
        return Err(("keys.default".into(), kind));
    }
    let up = match top.get("up") {
        None | Some(Value::Null) => Default::default(),
        Some(value) => groups(value, "up")?,
    };

    Ok(Manifest {
        name,
        author,
        license,
        keys,
        up,
    })
}

fn required_text(top: &Map<String, Value>, field: &str) -> Result<String, FieldError> {
    match top.get(field) {
        None | Some(Value::Null) => Err((field.into(), Kind::Missing)),
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.clone()),
        Some(_) => Err((field.into(), Kind::NotText)),
    }
}

/// The `keys` or `up` object: group name to a list of file names.
fn groups(value: &Value, field: &str) -> Result<[Vec<String>; 5], FieldError> {
    let Value::Object(map) = value else {
        return Err((field.into(), Kind::NotAnObject));
    };
    let mut groups: [Vec<String>; 5] = Default::default();
    // Sorted, so the same pack always reports the same first error.
    let mut names: Vec<&String> = map.keys().collect();
    names.sort();
    for name in names {
        let path = format!("{field}.{name}");
        let group = KeyGroup::from_name(name).ok_or_else(|| (path.clone(), Kind::UnknownGroup))?;
        let Value::Array(files) = &map[name] else {
            return Err((path, Kind::NotAList));
        };
        groups[group.index()] = files
            .iter()
            .enumerate()
            .map(|(i, file)| match file {
                Value::String(file) => Ok(file.clone()),
                _ => Err((format!("{path}[{i}]"), Kind::NotAFileName)),
            })
            .collect::<Result<_, _>>()?;
    }
    Ok(groups)
}
