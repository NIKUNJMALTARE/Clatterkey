//! The settings as the window sees and changes them. No Tauri here, so it
//! is tested directly.

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Map, Value, json};

use crate::audio::{Live, MAX_PITCH_VARIATION, OutputStatus, SoundSet, builtin_click};
use crate::packs::{self, Library, PackInfo};
use crate::permission::Permission;
use crate::settings::DEFAULT_PACK;
use crate::settings::{self, LoadProblem, Loaded, Settings};
use crate::startup::{default_sounds, load_pack};

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
    /// Where the bundled and the imported packs are.
    pub library: Library,
    /// The settings read at start-up, and what was wrong with the file.
    pub loaded: Loaded,
    /// The pack playing; `None` for the built-in click.
    pub playing: Option<String>,
    /// Whether the saved pack could not be used.
    pub fell_back: bool,
    /// The state the key path plays from.
    pub live: Arc<Live>,
    /// The macOS key-listening permission, as the watcher last found it.
    pub permission: Arc<Permission>,
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
    library: Library,
    /// Held for the whole of an import or a removal, so two of them never
    /// work on the imported folder at once. Always taken before `state`,
    /// and never by the key path.
    library_lock: Mutex<()>,
    live: Arc<Live>,
    permission: Arc<Permission>,
    audio: AudioStatus,
    /// The start-up problems, as `get_settings` reports them.
    startup: Value,
    state: Mutex<State>,
    /// Locked only while `state` is held, so it adds no lock order.
    listeners: Mutex<Vec<Listener>>,
}

/// The settings a patch may name. `launch_at_login` waits for its feature.
const CHANGEABLE: [&str; 5] = [
    "enabled",
    "volume",
    "pack_id",
    "pitch_variation",
    "key_up_sounds",
];

/// A checked change: only the fields the patch named.
#[derive(Default)]
struct Patch {
    enabled: Option<bool>,
    volume: Option<f64>,
    pack_id: Option<String>,
    pitch_variation: Option<f64>,
    key_up_sounds: Option<bool>,
}

impl Core {
    /// Puts the loaded `enabled`, `volume`, `pitch_variation` and
    /// `key_up_sounds` in force.
    pub fn new(parts: CoreParts) -> Self {
        let Loaded { settings, problem } = parts.loaded;
        parts.live.set_enabled(settings.enabled);
        parts.live.set_volume(settings.volume);
        parts.live.set_pitch_variation(settings.pitch_variation);
        parts.live.set_key_up(settings.key_up_sounds);
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
            library: parts.library,
            library_lock: Mutex::new(()),
            live: parts.live,
            permission: parts.permission,
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

    /// The state the key path plays from, for the preview, which plays
    /// from it too.
    pub fn live(&self) -> Arc<Live> {
        Arc::clone(&self.live)
    }

    /// Every bundled and imported pack whose `pack.json` can be read:
    /// `[{ id, name, author, license, has_key_up, imported }]`, sorted by
    /// name. No audio is opened.
    pub fn list_packs(&self) -> Value {
        self.library
            .list()
            .iter()
            .map(|listed| pack_json(&listed.info, listed.imported))
            .collect()
    }

    /// Imports the folder `source` as a pack, selects it and saves that
    /// choice: `{ pack, replaced, packs, change }`, where `change` is what
    /// `set_settings` would have returned. If the folder cannot be used
    /// nothing changes, and the error is the whole notice,
    /// `Could not import "<folder name>": <reason>`.
    pub fn import_pack(&self, source: &Path) -> Result<Value, String> {
        let _library = self.library_lock();
        // Checking and copying take a while, and the settings stay free
        // meanwhile: only putting the pack in force takes their lock.
        let imported = packs::import(source, &self.library).map_err(|error| {
            let folder = source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| source.display().to_string());
            format!("Could not import \"{folder}\": {error}")
        })?;
        let mut state = self.state();
        let patch = Patch {
            pack_id: Some(imported.info.id.clone()),
            ..Patch::default()
        };
        let change = self.put_in_force(&mut state, patch, Some(imported.sounds));
        Ok(json!({
            "pack": pack_json(&imported.info, true),
            "replaced": imported.replaced,
            "packs": self.list_packs(),
            "change": change,
        }))
    }

    /// Deletes the app's copy of the imported pack `id`: `{ packs, change }`.
    /// If it was playing, or was the saved pack, the default pack takes its
    /// place (the built-in click if that cannot load) and `change` is that
    /// change; otherwise `change` is null. A bundled pack, an unknown id
    /// and anything that is not an id are refused before any path is built.
    pub fn remove_pack(&self, id: &str) -> Result<Value, String> {
        let _library = self.library_lock();
        let mut state = self.state();
        if self.library.is_bundled(id) {
            return Err(format!("{id:?} is a bundled pack and cannot be removed"));
        }
        let found = self
            .library
            .find(id)
            .filter(|found| found.imported)
            .ok_or_else(|| unknown_pack(id))?;
        let folder = self
            .library
            .imported_dir()
            .ok_or_else(|| unknown_pack(id))?;

        // Moved aside in one step, so the pack is never listed half-deleted.
        let aside = folder.join(packs::removed_name(id));
        packs::rename(&found.dir, &aside).map_err(|error| error.to_string())?;
        // What cannot be deleted now is cleared at the next start-up.
        let _ = fs::remove_dir_all(&aside);

        let in_use = state.playing.as_deref() == Some(id) || state.settings.pack_id == id;
        let change = in_use.then(|| {
            let (sounds, playing) = match default_sounds(&self.library) {
                Ok(sounds) => (sounds, Some(DEFAULT_PACK.to_string())),
                Err(_) => (SoundSet::single(builtin_click()), None),
            };
            self.live.set_sounds(sounds);
            state.playing = playing;
            state.settings.pack_id = DEFAULT_PACK.to_string();
            self.finish(&mut state)
        });
        Ok(json!({ "packs": self.list_packs(), "change": change }))
    }

    fn library_lock(&self) -> MutexGuard<'_, ()> {
        self.library_lock.lock().unwrap_or_else(|p| p.into_inner())
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
            "permission": self.permission_status(),
            "revision": state.revision,
        })
    }

    /// `"not_needed"`, `"needed"`, `"granted"` or `"restart_needed"`. Reads
    /// an atomic: it never waits on the permission check.
    pub fn permission_status(&self) -> &'static str {
        self.permission.state().as_str()
    }

    /// `"playing"`, `"no_device"` or `"failed"`.
    pub fn audio_status(&self) -> &'static str {
        match (self.audio)() {
            OutputStatus::Playing => "playing",
            OutputStatus::NoDevice => "no_device",
            OutputStatus::Failed => "failed",
        }
    }

    /// Applies a change to `enabled`, `volume`, `pack_id`, `pitch_variation`
    /// and `key_up_sounds`, any of them, and saves it. The whole patch is checked first, and a new pack is
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
            let found = self.library.find(id).ok_or_else(|| unknown_pack(id))?;
            new_sounds = Some(load_pack(&found.dir)?);
        }
        Ok(self.put_in_force(state, patch, new_sounds))
    }

    /// Puts a checked patch in force, with `new_sounds` as the sounds of
    /// its pack if it names one, then saves and tells the listeners.
    /// Nothing here can fail.
    fn put_in_force(&self, state: &mut State, patch: Patch, new_sounds: Option<SoundSet>) -> Value {
        if let Some(enabled) = patch.enabled {
            state.settings.enabled = enabled;
            self.live.set_enabled(enabled);
        }
        if let Some(volume) = patch.volume {
            state.settings.volume = volume;
            self.live.set_volume(volume);
        }
        if let Some(variation) = patch.pitch_variation {
            state.settings.pitch_variation = variation;
            self.live.set_pitch_variation(variation);
        }
        if let Some(on) = patch.key_up_sounds {
            state.settings.key_up_sounds = on;
            self.live.set_key_up(on);
        }
        if let Some(id) = patch.pack_id {
            if let Some(sounds) = new_sounds {
                self.live.set_sounds(sounds);
            }
            state.playing = Some(id.clone());
            state.settings.pack_id = id;
        }
        self.finish(state)
    }

    /// The end of every change: saves the settings, raises the revision and
    /// tells the listeners. Returns the result.
    fn finish(&self, state: &mut State) -> Value {
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
        result
    }

    /// Checks every field of a patch, changing nothing.
    fn check(&self, patch: &Value) -> Result<Patch, String> {
        let Value::Object(map) = patch else {
            return Err("settings change: must be an object".to_string());
        };
        if let Some(other) = map.keys().find(|k| !CHANGEABLE.contains(&k.as_str())) {
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
            pitch_variation: field(
                map,
                "pitch_variation",
                |v| {
                    v.as_f64()
                        .filter(|n| (0.0..=MAX_PITCH_VARIATION).contains(n))
                },
                "must be a number from 0 to 0.3",
            )?,
            key_up_sounds: field(
                map,
                "key_up_sounds",
                |v| v.as_bool(),
                "must be true or false",
            )?,
        })
    }

    /// Whether `id` names a bundled or an imported pack's folder. The id is
    /// checked before any path is built from it.
    fn lists(&self, id: &str) -> bool {
        self.library.find(id).is_some()
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

/// A pack as the window lists it.
fn pack_json(info: &PackInfo, imported: bool) -> Value {
    json!({
        "id": info.id,
        "name": info.name,
        "author": info.author,
        "license": info.license,
        "has_key_up": info.has_key_up,
        "imported": imported,
    })
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
