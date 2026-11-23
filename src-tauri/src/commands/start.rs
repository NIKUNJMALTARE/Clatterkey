//! Builds the `Core` at start-up and publishes it for the window's
//! commands. Whatever goes wrong, a `Core` is published: a window whose
//! commands wait for ever would stay blank.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use crate::packs::Library;
use std::sync::Arc;

use super::{AudioStatus, Core, CoreGate, CoreParts};
use crate::audio::{Live, SoundSet, builtin_click};
use crate::permission::Permission;
use crate::settings::{self, Loaded, Settings};
use crate::startup::choose_pack_in;
use crate::{ErrorOut, write_line};

/// Reads the settings (creating the file if it is missing), chooses the
/// pack, and builds the `Core` over them. Returns it with the state the
/// key path plays from.
pub fn start_core(
    library: Library,
    settings_path: Option<PathBuf>,
    audio: AudioStatus,
    permission: Arc<Permission>,
    err: &ErrorOut,
) -> (Core, Arc<Live>) {
    let loaded = match &settings_path {
        Some(path) => settings::load_or_create_report(path, err),
        None => {
            write_line(
                err,
                "settings: the app config folder was not found; using defaults",
            );
            Loaded {
                settings: Settings::default(),
                problem: None,
            }
        }
    };
    // What an import or a removal left half-done in an earlier run.
    library.clean_temporary();
    let chosen = choose_pack_in(&library, &loaded.settings.pack_id, err);
    let live = Arc::new(Live::new(chosen.sounds));
    let core = Core::new(CoreParts {
        settings_path,
        library,
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::clone(&live),
        permission,
        audio,
    });
    (core, live)
}

/// A `Core` on the defaults and the built-in click, with nowhere to save
/// and no packs: what the window gets if start-up itself fails.
pub fn plain_core(audio: AudioStatus, permission: Arc<Permission>) -> (Core, Arc<Live>) {
    let live = Arc::new(Live::new(SoundSet::single(builtin_click())));
    let core = Core::new(CoreParts {
        settings_path: None,
        library: Library::none(),
        loaded: Loaded {
            settings: Settings::default(),
            problem: None,
        },
        playing: None,
        fell_back: true,
        live: Arc::clone(&live),
        permission,
        audio,
    });
    (core, live)
}

/// Runs `build` and publishes its `Core` to `gate`. If `build` panics, one
/// line is written and a plain `Core` is published in its place. Either
/// way the gate is open when this returns, and the returned state is the
/// one the published `Core` changes.
///
/// `audio` makes a status reader for whichever `Core` is published, and
/// `permission` is what the plain one reports.
pub fn publish_core(
    gate: &CoreGate,
    audio: impl Fn() -> AudioStatus,
    permission: Arc<Permission>,
    err: &ErrorOut,
    build: impl FnOnce(AudioStatus) -> (Core, Arc<Live>),
) -> Arc<Live> {
    publish_core_after(gate, audio, permission, err, build, |_| {})
}

/// As `publish_core`, running `before_publish` with the state the key path
/// plays from once the `Core` is built and before any command can reach it.
/// This is where the permission is first checked, so the window's first
/// read already has the right state and never shows the wrong view first.
pub fn publish_core_after(
    gate: &CoreGate,
    audio: impl Fn() -> AudioStatus,
    permission: Arc<Permission>,
    err: &ErrorOut,
    build: impl FnOnce(AudioStatus) -> (Core, Arc<Live>),
    before_publish: impl FnOnce(&Arc<Live>),
) -> Arc<Live> {
    let (core, live) = match catch_unwind(AssertUnwindSafe(|| build(audio()))) {
        Ok(built) => built,
        Err(_) => {
            write_line(err, "start-up failed; using the built-in click");
            plain_core(audio(), permission)
        }
    };
    // A panic here must not leave the gate shut and the window blank.
    if catch_unwind(AssertUnwindSafe(|| before_publish(&live))).is_err() {
        write_line(err, "start-up: the step before the window failed");
    }
    gate.publish(Arc::new(core));
    live
}
