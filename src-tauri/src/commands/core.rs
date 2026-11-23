//! The settings as the window sees and changes them. No Tauri here, so it
//! is tested directly.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Map, Value, json};

use crate::audio::{Live, OutputStatus, SoundSet};
use crate::packs;
use crate::settings::{self, LoadProblem, Loaded, Settings};
use crate::startup::{is_pack_id, load_pack, names_a_pack};

/// Reads the audio output's status now.
pub type AudioStatus = Box<dyn Fn() -> OutputStatus + Send + Sync>;

/// Told of every applied change, with the `set_settings` result. Listeners
/// run under the settings lock, in subscription order, so they see results
/// in revision order. They must return quickly and must not call back into
/// `Core`.
pub type Listener = Box<dyn Fn(&Value) + Send + Sync>;

/// What start-up hands over to the controller.
pub struct CoreParts {
    /// Where settings are saved; `None` if the config folder was not found.
    pub settings_path: Option<PathBuf>,
    /// The bundled packs folder; `None` if it was not found.
    pub packs: Option<PathBuf>,
    /// The settings read at start-up, and what was wrong with the file.
    pub loaded: Loaded,
    /// The pack playing; `None` for the built-in click.
    pub playing: Option<String>,
    /// Whether the saved pack could not be used.
    pub fell_back: bool,
    /// The state the key path plays from.
    pub live: Arc<Live>,
    pub audio: AudioStatus,
}

/// What changes after start-up. Held under one lock for a whole change, so
/// changes apply in the order they arrive and an older save can never land
/// after a newer one. The key path never takes this lock.
struct State {
    settings: Settings,
    playing: Option<String>,
    /// How many changes have been applied. It orders the results the
    /// window receives from command replies and from events.
    revision: u64,
}

/// Lists packs, reports the settings, and applies changes to them.
pub struct Core {
    settings_path: Option<PathBuf>,
    packs: Option<PathBuf>,
    live: Arc<Live>,
    audio: AudioStatus,
    /// The start-up problems, as `get_settings` reports them.
    startup: Value,
    state: Mutex<State>,
    /// Locked only while `state` is held, so it adds no lock order.
    listeners: Mutex<Vec<Listener>>,
}

/// A checked change: only the fields the patch named.
#[derive(Default)]
struct Patch {
    enabled: Option<bool>,
    volume: Option<f64>,
    pack_id: Option<String>,
}

impl Core {
    /// Puts the loaded `enabled` and `volume` in force.
    pub fn new(parts: CoreParts) -> Self {
        let Loaded { settings, problem } = parts.loaded;
        parts.live.set_enabled(settings.enabled);
        parts.live.set_volume(settings.volume);
        let replaced: Vec<&str> = match &problem {
            Some(LoadProblem::Replaced(fields)) => fields.clone(),
            _ => Vec::new(),
        };
        let startup = json!({
            "settings_unreadable": matches!(problem, Some(LoadProblem::Unreadable)),
            "replaced_fields": replaced,
            "unusable_pack": parts.fell_back.then(|| settings.pack_id.clone()),
        });
        Self {
            settings_path: parts.settings_path,
            packs: parts.packs,
            live: parts.live,
            audio: parts.audio,
            startup,
            state: Mutex::new(State {
                settings,
                playing: parts.playing,
                revision: 0,
            }),
            listeners: Mutex::new(Vec::new()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // The state is plain values, replaced whole, so it is usable after
        // a panic elsewhere.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Every pack in the packs folder whose `pack.json` can be read:
    /// `[{ id, name, author, license }]`, sorted by name. No audio is opened.
    pub fn list_packs(&self) -> Value {
        let mut found = Vec::new();
        if let Some(dir) = &self.packs
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(id) = name.to_str().filter(|id| is_pack_id(id)) else {
                    continue;
                };
                if let Ok(info) = packs::read_info(&dir.join(id)) {
                    found.push(info);
                }
            }
        }
        found.sort_by(|a, b| {
            (a.name.to_lowercase(), &a.name, &a.id).cmp(&(b.name.to_lowercase(), &b.name, &b.id))
        });
        found
            .into_iter()
            .map(|info| {
                json!({
                    "id": info.id,
                    "name": info.name,
                    "author": info.author,
                    "license": info.license,
                })
            })
            .collect()
    }

    /// What the window opens with: the settings, the pack actually playing,
    /// the start-up problems and the audio output's status.
    pub fn get_settings(&self) -> Value {
        let state = self.state();
        json!({
            "settings": settings::to_value(&state.settings),
            "playing_pack": state.playing,
            "startup": self.startup,
            "audio": self.audio_status(),
            "revision": state.revision,
        })
    }

    /// `"playing"`, `"no_device"` or `"failed"`.
    pub fn audio_status(&self) -> &'static str {
        match (self.audio)() {
            OutputStatus::Playing => "playing",
            OutputStatus::NoDevice => "no_device",
            OutputStatus::Failed => "failed",
        }
    }

    /// Applies a change to `enabled`, `volume` and `pack_id`, any of them,
    /// and saves it. The whole patch is checked first, and a new pack is
    /// loaded before anything changes, so a rejected patch changes nothing
    /// and writes nothing. A save that fails is not an error: the change is
    /// in force and the result says it was not saved.
    pub fn set_settings(&self, patch: &Value) -> Result<Value, String> {
        let mut state = self.state();
        let patch = self.check(patch)?;
        self.apply(&mut state, patch)
    }

    /// Turns the sound off if it is on and on if it is off, by the same
    /// path as `set_settings`, and returns the same result. The read and
    /// the change are one step, so two toggles never read the same value.
    pub fn toggle_enabled(&self) -> Value {
        let mut state = self.state();
        let patch = Patch {
            enabled: Some(!state.settings.enabled),
            ..Patch::default()
        };
        // Only a pack change can be refused.
        self.apply(&mut state, patch)
            .unwrap_or_else(|_| result_of(&state, None))
    }

    /// Adds a listener for applied changes, and calls it once now with the
    /// settings in force, so it starts in step.
    pub fn subscribe(&self, listener: Listener) {
        let state = self.state();
        call(&listener, &result_of(&state, None));
        self.listeners().push(listener);
    }

    fn listeners(&self) -> MutexGuard<'_, Vec<Listener>> {
        self.listeners.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Applies a checked patch, saves, and tells the listeners.
    fn apply(&self, state: &mut State, patch: Patch) -> Result<Value, String> {
        // The old pack plays until the new one is loaded whole.
        let mut new_sounds: Option<SoundSet> = None;
        if let Some(id) = &patch.pack_id
            && state.playing.as_deref() != Some(id.as_str())
        {
            let dir = self.packs.as_deref().ok_or_else(|| unknown_pack(id))?;
            new_sounds = Some(load_pack(&dir.join(id))?);
        }

        if let Some(enabled) = patch.enabled {
            state.settings.enabled = enabled;
            self.live.set_enabled(enabled);
        }
        if let Some(volume) = patch.volume {
            state.settings.volume = volume;
            self.live.set_volume(volume);
        }
        if let Some(id) = patch.pack_id {
            if let Some(sounds) = new_sounds {
                self.live.set_sounds(sounds);
            }
            state.playing = Some(id.clone());
            state.settings.pack_id = id;
        }

        let save_error = match &self.settings_path {
            Some(path) => settings::save(path, &state.settings)
                .err()
                .map(|e| e.to_string()),
            None => Some("the app config folder was not found".to_string()),
        };
        state.revision += 1;
        let result = result_of(state, save_error);
        for listener in self.listeners().iter() {
            call(listener, &result);
        }
        Ok(result)
    }

    /// Checks every field of a patch, changing nothing.
    fn check(&self, patch: &Value) -> Result<Patch, String> {
        let Value::Object(map) = patch else {
            return Err("settings change: must be an object".to_string());
        };
        if let Some(other) = map
            .keys()
            .find(|k| !matches!(k.as_str(), "enabled" | "volume" | "pack_id"))
        {
            return Err(format!("settings change: {other:?} cannot be changed here"));
        }
        Ok(Patch {
            enabled: field(map, "enabled", |v| v.as_bool(), "must be true or false")?,
            volume: field(
                map,
                "volume",
                |v| v.as_f64().filter(|n| (0.0..=1.0).contains(n)),
                "must be a number from 0 to 1",
            )?,
            pack_id: match map.get("pack_id") {
                None => None,
                Some(Value::String(id)) if self.lists(id) => Some(id.clone()),
                Some(Value::String(id)) => return Err(unknown_pack(id)),
                Some(_) => return Err("settings change: pack_id must be a pack's id".to_string()),
            },
        })
    }

    /// Whether `id` names a folder in the packs folder. The id is checked
    /// before any path is built from it.
    fn lists(&self, id: &str) -> bool {
        self.packs
            .as_deref()
            .is_some_and(|dir: &Path| is_pack_id(id) && names_a_pack(dir, id))
    }
}

/// The result of a change: the settings in force and how the save went.
fn result_of(state: &State, save_error: Option<String>) -> Value {
    json!({
        "settings": settings::to_value(&state.settings),
        "playing_pack": state.playing,
        "saved": save_error.is_none(),
        "save_error": save_error,
        "revision": state.revision,
    })
}

/// Calls a listener. One that panics must not undo or block the change.
fn call(listener: &Listener, result: &Value) {
    let _ = catch_unwind(AssertUnwindSafe(|| listener(result)));
}

fn unknown_pack(id: &str) -> String {
    format!("unknown sound pack {id:?}")
}

/// One optional field of a patch, read with `read`, which gives `None` for
/// a value of the wrong type or out of range.
fn field<T>(
    map: &Map<String, Value>,
    name: &str,
    read: impl Fn(&Value) -> Option<T>,
    must: &str,
) -> Result<Option<T>, String> {
    match map.get(name) {
        None => Ok(None),
        Some(value) => read(value)
            .map(Some)
            .ok_or_else(|| format!("settings change: {name} {must}")),
    }
}
