//! Builds the `Core` at start-up and publishes it for the window's
//! commands. Whatever goes wrong, a `Core` is published: a window whose
//! commands wait for ever would stay blank.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use super::{AudioStatus, Core, CoreGate, CoreParts};
use crate::audio::{Live, SoundSet, builtin_click};
use crate::settings::{self, Loaded, Settings};
use crate::startup::choose_pack;
use crate::{ErrorOut, write_line};

/// Reads the settings (creating the file if it is missing), chooses the
/// pack, and builds the `Core` over them. Returns it with the state the
/// key path plays from.
pub fn start_core(
    packs: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    audio: AudioStatus,
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
    let chosen = choose_pack(packs.as_deref(), &loaded.settings.pack_id, err);
    let live = Arc::new(Live::new(chosen.sounds));
    let core = Core::new(CoreParts {
        settings_path,
        packs,
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        live: Arc::clone(&live),
        audio,
    });
    (core, live)
}

/// A `Core` on the defaults and the built-in click, with nowhere to save
/// and no packs: what the window gets if start-up itself fails.
pub fn plain_core(audio: AudioStatus) -> (Core, Arc<Live>) {
    let live = Arc::new(Live::new(SoundSet::single(builtin_click())));
    let core = Core::new(CoreParts {
        settings_path: None,
        packs: None,
        loaded: Loaded {
            settings: Settings::default(),
            problem: None,
        },
        playing: None,
        fell_back: true,
        live: Arc::clone(&live),
        audio,
    });
    (core, live)
}

/// Runs `build` and publishes its `Core` to `gate`. If `build` panics, one
/// line is written and a plain `Core` is published in its place. Either
/// way the gate is open when this returns, and the returned state is the
/// one the published `Core` changes.
///
/// `audio` makes a status reader for whichever `Core` is published.
pub fn publish_core(
    gate: &CoreGate,
    audio: impl Fn() -> AudioStatus,
    err: &ErrorOut,
    build: impl FnOnce(AudioStatus) -> (Core, Arc<Live>),
) -> Arc<Live> {
    let (core, live) = match catch_unwind(AssertUnwindSafe(|| build(audio()))) {
        Ok(built) => built,
        Err(_) => {
            write_line(err, "start-up failed; using the built-in click");
            plain_core(audio())
        }
    };
    gate.publish(Arc::new(core));
    live
}
