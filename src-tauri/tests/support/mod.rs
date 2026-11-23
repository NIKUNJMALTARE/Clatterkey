//! A `Core` over temporary files and fixture packs, with a `Player` that
//! plays into a recording sink. Kept apart from `common` so that the earlier
//! tests, which share `common`, do not depend on the new `commands` module.
//!
//! Interfaces assumed from the feature 06 plan:
//!
//! ```ignore
//! // audio
//! pub struct Live; Live::new(SoundSet), set_enabled, enabled, set_volume(f64), gain() -> f32
//! Player::with_live(Arc<dyn Sink>, Arc<Live>)
//! // settings
//! pub struct Loaded { pub settings: Settings, pub problem: Option<LoadProblem> }
//! pub enum LoadProblem { Unreadable, Replaced(Vec<&'static str>) }
//! pub fn load_or_create_report(path: &Path, err: &ErrorOut) -> Loaded;
//! // startup
//! pub struct Chosen { pub sounds: SoundSet, pub playing: Option<String>, pub fell_back: bool }
//! pub fn choose_pack(packs: Option<&Path>, pack_id: &str, err: &ErrorOut) -> Chosen;
//! // commands
//! Core::new(CoreParts { settings_path, packs, loaded, playing, fell_back, live, audio })
//! ```

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use mechanical_key_sounds_lib::audio::{Live, OutputStatus, Player};
use mechanical_key_sounds_lib::commands::{Core, CoreParts};
use mechanical_key_sounds_lib::packs::Library;
use mechanical_key_sounds_lib::permission::{Permission, PermissionState};
use mechanical_key_sounds_lib::settings;
use mechanical_key_sounds_lib::startup::choose_pack;

use crate::common::*;

pub struct Rig {
    pub core: Arc<Core>,
    pub live: Arc<Live>,
    pub sink: Arc<RecordingSink>,
    pub player: Player,
    pub settings_path: PathBuf,
    pub packs_path: PathBuf,
    pub err: Captured,
    _dir: TempDir,
    _packs: TempDir,
}

/// A complete settings file with values that differ from the defaults
/// in the fields the window does not change.
pub fn file(pack_id: &str) -> String {
    format!(
        "{{\n  \"enabled\": true,\n  \"pack_id\": {pack_id:?},\n  \"volume\": 0.8,\n  \"pitch_variation\": 0.2,\n  \"key_up_sounds\": false,\n  \"launch_at_login\": true\n}}\n"
    )
}

/// Starts as the app does: `file` is the settings file (`None`: no file),
/// `packs` the `(id, fixture)` folders copied into the packs folder.
pub fn rig(file: Option<&str>, packs: &[(&str, &str)]) -> Rig {
    rig_with(file, packs, |_| ())
}

/// As `rig`, with `prepare` run on the packs folder before start-up.
pub fn rig_with(
    file: Option<&str>,
    packs: &[(&str, &str)],
    prepare: impl FnOnce(&std::path::Path),
) -> Rig {
    let dir = TempDir::new("cfg");
    let packs_dir = packs_folder("packs", packs);
    prepare(packs_dir.path());
    let settings_path = dir.path().join("settings.json");
    if let Some(text) = file {
        fs::write(&settings_path, text).unwrap();
    }
    let (err, err_out) = fake_err();
    let loaded = settings::load_or_create_report(&settings_path, &err_out);
    let chosen = choose_pack(Some(packs_dir.path()), &loaded.settings.pack_id, &err_out);
    let live = Arc::new(Live::new(chosen.sounds));
    let core = Arc::new(Core::new(CoreParts {
        settings_path: Some(settings_path.clone()),
        library: Library::bundled_only(packs_dir.path().to_path_buf()),
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::clone(&live),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    }));
    let sink = Arc::new(RecordingSink::default());
    let player = Player::with_live(sink.clone(), Arc::clone(&live));
    Rig {
        core,
        live,
        sink,
        player,
        settings_path,
        packs_path: packs_dir.path().to_path_buf(),
        err,
        _dir: dir,
        _packs: packs_dir,
    }
}

impl Rig {
    pub fn settings(&self) -> settings::Settings {
        settings::read(&self.settings_path).unwrap().settings
    }

    pub fn bytes(&self) -> Option<Vec<u8>> {
        fs::read(&self.settings_path).ok()
    }
}
