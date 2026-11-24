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

use clatterkey_lib::audio::{Live, OutputStatus, Player};
use clatterkey_lib::autostart::{Inert, LoginItem};
use clatterkey_lib::commands::{Core, CoreParts};
use clatterkey_lib::packs::Library;
use clatterkey_lib::permission::{Permission, PermissionState};
use clatterkey_lib::settings;
use clatterkey_lib::startup::choose_pack;

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
    rig_item(file, packs, prepare, Arc::new(Inert))
}

/// As `rig_with`, over the given login registration.
pub fn rig_item(
    file: Option<&str>,
    packs: &[(&str, &str)],
    prepare: impl FnOnce(&std::path::Path),
    login_item: Arc<dyn LoginItem>,
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
        login_item,
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

/// A login registration that records what it is asked and can be told what
/// to answer. `enable` puts one in force and `disable` takes it out, as the
/// system would, unless told to fail.
pub struct FakeItem {
    pub calls: std::sync::Mutex<Vec<&'static str>>,
    pub in_force: std::sync::Mutex<Result<bool, String>>,
    pub enable_fails: std::sync::Mutex<Option<String>>,
    pub disable_fails: std::sync::Mutex<Option<String>>,
    pub tracks: bool,
}

impl FakeItem {
    pub fn new(in_force: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: Default::default(),
            in_force: std::sync::Mutex::new(Ok(in_force)),
            enable_fails: Default::default(),
            disable_fails: Default::default(),
            tracks: true,
        })
    }

    pub fn untracked() -> Arc<Self> {
        Arc::new(Self {
            calls: Default::default(),
            in_force: std::sync::Mutex::new(Ok(false)),
            enable_fails: Default::default(),
            disable_fails: Default::default(),
            tracks: false,
        })
    }

    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }

    pub fn fail_enable(&self, why: &str) {
        *self.enable_fails.lock().unwrap() = Some(why.to_string());
    }

    pub fn fail_disable(&self, why: &str) {
        *self.disable_fails.lock().unwrap() = Some(why.to_string());
    }

    pub fn cannot_be_asked(&self, why: &str) {
        *self.in_force.lock().unwrap() = Err(why.to_string());
    }
}

impl LoginItem for FakeItem {
    fn enable(&self) -> Result<(), String> {
        self.calls.lock().unwrap().push("enable");
        if let Some(why) = self.enable_fails.lock().unwrap().clone() {
            return Err(why);
        }
        *self.in_force.lock().unwrap() = Ok(true);
        Ok(())
    }

    fn disable(&self) -> Result<(), String> {
        self.calls.lock().unwrap().push("disable");
        if let Some(why) = self.disable_fails.lock().unwrap().clone() {
            return Err(why);
        }
        *self.in_force.lock().unwrap() = Ok(false);
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, String> {
        self.calls.lock().unwrap().push("is_enabled");
        self.in_force.lock().unwrap().clone()
    }

    fn tracks_system(&self) -> bool {
        self.tracks
    }
}
