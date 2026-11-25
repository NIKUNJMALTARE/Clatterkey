//! Feature 10, the controller side (AC13, AC17 settings half, AC18, AC21 to
//! AC24, AC26) over `Core`, a fake sink and a recording subscriber, on
//! temporary folders.
//!
//! Interfaces assumed from the feature 10 plan:
//!
//! ```ignore
//! Core::import_pack(&self, &Path) -> Result<Value, String>
//!   // Ok: { pack: {.., imported: true}, replaced, packs: [..], change: {..} }
//!   // Err: `Could not import "<folder name>": <reason>`
//! Core::remove_pack(&self, id: &str) -> Result<Value, String>
//!   // Ok: { packs: [..], change: <result> | null }
//! Core::list_packs(): each entry gains "imported"
//! start_core(Library, settings_path, audio, permission, login_item, err)
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use clatterkey_lib::audio::{Live, OutputStatus, Player, builtin_click};
use clatterkey_lib::autostart::Inert;
use clatterkey_lib::commands::{Core, CoreParts, start_core};
use clatterkey_lib::keys::KeyCode;
use clatterkey_lib::packs::Library;
use clatterkey_lib::permission::{Permission, PermissionState};
use clatterkey_lib::settings;
use clatterkey_lib::startup::choose_pack_in;
use serde_json::{Value, json};

mod common;
use common::*;
mod support;
use support::file;

/// A library over a bundled folder and an imported folder (which need not
/// exist yet).
fn library(bundled: &Path, imported: &Path) -> Library {
    Library::new(Some(bundled.to_path_buf()), Some(imported.to_path_buf()))
}

/// Bundled `brown` (fixture default-only) and `blue` (fixture full); the
/// imported folder is `<data>/packs`.
struct Rig {
    core: Arc<Core>,
    live: Arc<Live>,
    sink: Arc<RecordingSink>,
    player: Arc<Player>,
    settings_path: PathBuf,
    imported: PathBuf,
    bundled: PathBuf,
    told: Arc<Mutex<Vec<Value>>>,
    _data: TempDir,
    _bundled: TempDir,
}

fn rig(saved_pack: &str) -> Rig {
    rig_with(
        saved_pack,
        &[("brown", "default-only"), ("blue", "full")],
        |_| (),
    )
}

fn rig_with(saved_pack: &str, bundled: &[(&str, &str)], prepare: impl FnOnce(&Path)) -> Rig {
    let data = TempDir::new("data");
    let bundled_dir = packs_folder("bundled", bundled);
    let imported = data.path().join("packs");
    prepare(&imported);
    let settings_path = data.path().join("settings.json");
    fs::write(&settings_path, file(saved_pack)).unwrap();
    let library = library(bundled_dir.path(), &imported);
    let (_, err) = fake_err();
    let loaded = settings::load_or_create_report(&settings_path, &err);
    let chosen = choose_pack_in(&library, &loaded.settings.pack_id, &err);
    let live = Arc::new(Live::new(chosen.sounds));
    let core = Arc::new(Core::new(CoreParts {
        settings_path: Some(settings_path.clone()),
        library,
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        login_item: Arc::new(Inert),
        live: Arc::clone(&live),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    }));
    let told = Arc::new(Mutex::new(Vec::new()));
    let t = Arc::clone(&told);
    core.subscribe(Box::new(move |v: &Value| t.lock().unwrap().push(v.clone())));
    let sink = Arc::new(RecordingSink::default());
    let player = Arc::new(Player::with_live(sink.clone(), Arc::clone(&live)));
    Rig {
        core,
        live,
        sink,
        player,
        settings_path,
        imported,
        bundled: bundled_dir.path().to_path_buf(),
        told,
        _data: data,
        _bundled: bundled_dir,
    }
}

/// The lengths of the `default-only` fixture's three default sounds, which
/// the bundled `brown` is in these tests. One is chosen at random per key.
const BROWN_MS: [u64; 3] = [40, 45, 50];

fn assert_brown_plays(rig: &Rig, why: &str) {
    for _ in 0..10 {
        let got = rig.letter_ms();
        assert!(BROWN_MS.contains(&got), "{why}: played {got} ms");
    }
}

impl Rig {
    fn revision(&self) -> u64 {
        self.core.get_settings()["revision"].as_u64().unwrap()
    }

    fn saved(&self) -> settings::Settings {
        settings::read(&self.settings_path).unwrap().settings
    }

    fn told(&self) -> usize {
        self.told.lock().unwrap().len()
    }

    /// The length in ms of the sound a letter key plays now.
    fn letter_ms(&self) -> u64 {
        let before = self.sink.count();
        self.player.handle(down(KeyCode::KeyA));
        let samples = self.sink.samples();
        assert_eq!(samples.len(), before + 1, "a key plays one sound");
        ms(&samples[before])
    }

    fn space_ms(&self) -> u64 {
        let before = self.sink.count();
        self.player.handle(down(KeyCode::Space));
        ms(&self.sink.samples()[before])
    }

    fn ids(&self) -> Vec<String> {
        self.core
            .list_packs()
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect()
    }
}

fn import_ok(rig: &Rig, src: &Path) -> Value {
    rig.core
        .import_pack(src)
        .unwrap_or_else(|e| panic!("import refused: {e}"))
}

// ---- AC13: a successful import is a change ----

#[test]
fn ac13_the_imported_pack_plays_from_the_next_key_and_is_saved() {
    let rig = rig("brown");
    let brown_ms = rig.letter_ms();
    let revision = rig.revision();
    let told = rig.told();
    let before = rig.saved();
    let (_t, src) = source_with_lengths("My Clicks", 33, Some(77));

    let v = import_ok(&rig, &src);

    assert_eq!(v["pack"]["id"], "my-clicks");
    assert_eq!(v["pack"]["name"], "Mine 33");
    assert_eq!(v["pack"]["imported"], true);
    assert_eq!(v["replaced"], false);
    assert_eq!(v["change"]["playing_pack"], "my-clicks");
    assert_eq!(v["change"]["settings"]["pack_id"], "my-clicks");
    assert_eq!(v["change"]["saved"], true);
    assert_eq!(v["change"]["revision"], revision + 1);
    let listed: Vec<&str> = v["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(listed.contains(&"my-clicks"));

    assert_eq!(rig.letter_ms(), 33);
    assert_eq!(rig.space_ms(), 77);
    assert_ne!(brown_ms, 33);

    let after = rig.saved();
    assert_eq!(after.pack_id, "my-clicks");
    assert_eq!(after.enabled, before.enabled);
    assert_eq!(after.volume, before.volume);
    assert_eq!(after.pitch_variation, before.pitch_variation);
    assert_eq!(after.key_up_sounds, before.key_up_sounds);
    assert_eq!(after.launch_at_login, before.launch_at_login);

    assert_eq!(rig.revision(), revision + 1);
    assert_eq!(rig.told(), told + 1, "subscribers are told once");
    assert_eq!(rig.core.get_settings()["playing_pack"], "my-clicks");
}

#[test]
fn ac13_the_other_settings_in_force_are_untouched_by_an_import() {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("mine", 33, None);
    let v = import_ok(&rig, &src);
    assert_eq!(v["change"]["settings"]["volume"], 0.8);
    assert_eq!(v["change"]["settings"]["pitch_variation"], 0.2);
    assert_eq!(v["change"]["settings"]["key_up_sounds"], false);
    assert!(rig.live.enabled());
}

#[test]
fn ac13_a_failed_save_is_reported_in_the_change_and_the_pack_still_plays() {
    let rig = rig("brown");
    // Make the settings file's place unwritable: a folder where the file is.
    fs::remove_file(&rig.settings_path).unwrap();
    fs::create_dir_all(rig.settings_path.join("blocked")).unwrap();
    let (_t, src) = source_with_lengths("mine", 33, None);
    let v = import_ok(&rig, &src);
    assert_eq!(v["change"]["saved"], false);
    assert!(!v["change"]["save_error"].as_str().unwrap().is_empty());
    assert_eq!(rig.letter_ms(), 33);
}

// ---- AC17: a refused import changes nothing ----

#[test]
fn ac17_a_refused_import_reports_the_notice_text_and_changes_nothing() {
    let rig = rig("brown");
    let bytes = fs::read(&rig.settings_path).unwrap();
    let revision = rig.revision();
    let told = rig.told();
    let bad = TestPack::new("bad-one", &json_default(&["a.wav"]));

    let err = rig.core.import_pack(&bad.dir).expect_err("refused");

    assert_eq!(
        err,
        "Could not import \"bad-one\": keys.default[0] \"a.wav\": file not found"
    );
    assert_eq!(fs::read(&rig.settings_path).unwrap(), bytes);
    assert_eq!(rig.revision(), revision);
    assert_eq!(rig.told(), told);
    assert_eq!(rig.core.get_settings()["playing_pack"], "brown");
    assert_brown_plays(&rig, "brown still plays");
    assert!(!rig.imported.exists() || fs::read_dir(&rig.imported).unwrap().count() == 0);
    assert_eq!(rig.ids(), ["brown", "blue"]);
}

#[test]
fn ac17_the_folder_name_in_the_notice_is_written_as_it_is_and_messages_carry_no_keys() {
    let rig = rig("brown");
    let tmp = TempDir::new("src");
    let src = tmp.path().join("Old Board");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("notes.txt"), "x").unwrap();
    let err = rig.core.import_pack(&src).expect_err("refused");
    assert_eq!(
        err,
        "Could not import \"Old Board\": no audio files found (WAV, OGG, MP3 or FLAC)"
    );
}

#[test]
fn ac17_a_folder_that_does_not_exist_is_refused_with_the_notice_text() {
    let rig = rig("brown");
    let err = rig
        .core
        .import_pack(Path::new("/definitely/not/here/Nowhere"))
        .expect_err("refused");
    assert!(err.starts_with("Could not import \"Nowhere\": "), "{err}");
}

// ---- AC18: importing again ----

#[test]
fn ac18_importing_the_playing_pack_again_plays_the_new_sounds_from_the_next_key() {
    let rig = rig("brown");
    let (_t1, first) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &first);
    assert_eq!(rig.letter_ms(), 33);
    let revision = rig.revision();
    let told = rig.told();

    let (_t2, second) = source_with_lengths("my-clicks", 55, None);
    let v = import_ok(&rig, &second);

    assert_eq!(v["replaced"], true);
    assert_eq!(rig.letter_ms(), 55, "the new sounds are in force");
    assert_eq!(rig.revision(), revision + 1);
    assert_eq!(rig.told(), told + 1);
    assert_eq!(rig.ids().iter().filter(|i| *i == "my-clicks").count(), 1);
}

#[test]
fn ac18_a_refused_replacement_keeps_the_old_sounds_playing() {
    let rig = rig("brown");
    let (_t1, first) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &first);
    let revision = rig.revision();
    let before = snapshot(&rig.imported);
    let bad = TestPack::new("my-clicks", &json_default(&["missing.wav"]));
    assert!(rig.core.import_pack(&bad.dir).is_err());
    assert_eq!(rig.letter_ms(), 33);
    assert_eq!(rig.revision(), revision);
    assert_eq!(snapshot(&rig.imported), before);
}

#[test]
fn ac18_a_bundled_id_is_not_replaced_by_an_import() {
    let rig = rig("brown");
    let brown_ms = rig.letter_ms();
    let (_t, src) = source_with_lengths("brown", 33, None);
    let v = import_ok(&rig, &src);
    assert_eq!(v["pack"]["id"], "brown-2");
    assert_eq!(v["replaced"], false);
    assert_eq!(rig.letter_ms(), 33);
    assert!(rig.bundled.join("brown/pack.json").is_file());
    assert_ne!(brown_ms, 33);
}

// ---- Feature 15 AC12: imported packs with a new bundled id ----

fn real_bundled() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("packs")
}

fn real_core(imported: &Path) -> Core {
    let data = imported.parent().unwrap().to_path_buf();
    let settings_path = data.join("settings.json");
    fs::write(&settings_path, file("brown")).unwrap();
    let library = library(&real_bundled(), imported);
    let (_, err) = fake_err();
    let loaded = settings::load_or_create_report(&settings_path, &err);
    let chosen = choose_pack_in(&library, &loaded.settings.pack_id, &err);
    Core::new(CoreParts {
        settings_path: Some(settings_path),
        library,
        loaded,
        playing: chosen.playing,
        fell_back: chosen.fell_back,
        login_item: Arc::new(Inert),
        live: Arc::new(Live::new(chosen.sounds)),
        permission: Arc::new(Permission::new(PermissionState::NotNeeded)),
        audio: Box::new(|| OutputStatus::Playing),
    })
}

#[test]
fn feature_15_ac12_an_imported_folder_with_a_new_bundled_id_is_hidden_and_kept() {
    for id in [
        "thocky-linear",
        "fast-linear",
        "sharp-clicky",
        "tactile-bump",
    ] {
        let data = TempDir::new("f15");
        let imported = data.path().join("packs");
        copy_dir(&fixture_pack("extra-field"), &imported.join(id));
        let core = real_core(&imported);
        let before = snapshot(&imported);

        let list = core.list_packs();
        let rows: Vec<&Value> = list
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"] == id)
            .collect();
        assert_eq!(rows.len(), 1, "pack {id} is listed once");
        assert_eq!(rows[0]["imported"], false, "pack {id}: the bundled one");
        assert_ne!(rows[0]["name"], "Extra field");

        let found = library(&real_bundled(), &imported).find(id).expect("found");
        assert!(!found.imported, "find({id}) gives the bundled pack");
        assert_eq!(found.dir, real_bundled().join(id));

        assert!(imported.join(id).join("pack.json").is_file());
        assert_eq!(snapshot(&imported), before, "the imported folder is kept");
    }
}

#[test]
fn feature_15_ac12_importing_a_folder_named_fast_linear_gives_fast_linear_2() {
    let data = TempDir::new("f15-import");
    let imported = data.path().join("packs");
    let core = real_core(&imported);
    let (_t, src) = source_with_lengths("Fast linear", 33, None);
    let v = core
        .import_pack(&src)
        .unwrap_or_else(|e| panic!("import refused: {e}"));
    assert_eq!(v["pack"]["id"], "fast-linear-2");
    assert_eq!(v["replaced"], false);
    assert!(imported.join("fast-linear-2/pack.json").is_file());
    let list = core.list_packs();
    let bundled_row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "fast-linear")
        .expect("bundled fast-linear listed");
    assert_eq!(bundled_row["imported"], false);
}

// ---- AC20: list_packs over two folders ----

#[test]
fn ac20_list_packs_returns_both_sorted_by_name_with_imported_flags() {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &src);
    // Not listed: an id that is not valid, a shadow of a bundled id, a
    // temporary folder.
    copy_dir(&fixture_pack("full"), &rig.imported.join("has space"));
    copy_dir(&fixture_pack("extra-field"), &rig.imported.join("brown"));
    copy_dir(&fixture_pack("full"), &rig.imported.join(".import-z-1-1"));

    let list = rig.core.list_packs();
    let rows: Vec<(&str, &str, bool)> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap(),
                p["name"].as_str().unwrap(),
                p["imported"].as_bool().expect("every entry has imported"),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("brown", "Default only", false),
            ("blue", "Full test pack", false),
            ("my-clicks", "Mine 33", true),
        ]
    );
    for row in list.as_array().unwrap() {
        assert!(
            row.get("author").is_some()
                && row.get("license").is_some()
                && row.get("has_key_up").is_some()
        );
    }
}

#[test]
fn ac20_a_pack_put_in_the_imported_folder_by_hand_is_listed() {
    let rig = rig("brown");
    copy_dir(&fixture_pack("extra-field"), &rig.imported.join("by-hand"));
    let list = rig.core.list_packs();
    let hand = list
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "by-hand")
        .expect("listed");
    assert_eq!(hand["imported"], true);
}

// ---- AC21: choosing and starting with an imported pack ----

#[test]
fn ac21_set_settings_accepts_an_imported_pack_id_and_plays_it() {
    let rig = rig("brown");
    copy_dir(&fixture_pack("full"), &rig.imported.join("by-hand"));
    let r = rig
        .core
        .set_settings(&json!({"pack_id": "by-hand"}))
        .unwrap();
    assert_eq!(r["playing_pack"], "by-hand");
    assert_eq!(rig.space_ms(), 70, "the full fixture's space sound");
    assert_eq!(rig.saved().pack_id, "by-hand");
}

#[test]
fn ac21_set_settings_still_refuses_ids_that_name_no_pack() {
    let rig = rig("brown");
    let revision = rig.revision();
    for id in ["nope", "../x", "", "/abs"] {
        assert!(
            rig.core.set_settings(&json!({ "pack_id": id })).is_err(),
            "{id:?}"
        );
    }
    assert_eq!(rig.revision(), revision);
}

#[test]
fn ac21_a_saved_imported_pack_plays_from_the_first_key_event_at_start_up() {
    let rig = rig_with("mine", &[("brown", "default-only")], |imported| {
        let (_t, src) = source_with_lengths("mine", 44, None);
        copy_dir(&src, &imported.join("mine"));
    });
    let v = rig.core.get_settings();
    assert_eq!(v["playing_pack"], "mine");
    assert!(v["startup"]["unusable_pack"].is_null());
    assert_eq!(rig.letter_ms(), 44);
}

#[test]
fn ac21_start_core_with_a_library_plays_the_saved_imported_pack() {
    let data = TempDir::new("data");
    let bundled = packs_folder("bundled", &[("brown", "default-only")]);
    let imported = data.path().join("packs");
    let (_t, src) = source_with_lengths("mine", 44, None);
    copy_dir(&src, &imported.join("mine"));
    // A leftover temporary folder from a crashed import is cleaned up.
    copy_dir(&fixture_pack("full"), &imported.join(".import-mine-1-1"));
    let path = data.path().join("settings.json");
    fs::write(&path, file("mine")).unwrap();
    let (_, err) = fake_err();

    let (core, live) = start_core(
        library(bundled.path(), &imported),
        Some(path),
        Box::new(|| OutputStatus::Playing),
        Arc::new(Permission::new(PermissionState::NotNeeded)),
        Arc::new(Inert),
        &err,
    );

    assert_eq!(core.get_settings()["playing_pack"], "mine");
    let sink = Arc::new(RecordingSink::default());
    Player::with_live(sink.clone(), live).handle(down(KeyCode::KeyA));
    assert_eq!(ms(&sink.samples()[0]), 44);
    assert!(!imported.join(".import-mine-1-1").exists());
}

#[test]
fn ac21_a_deleted_imported_pack_falls_back_as_in_feature_05_and_is_reported() {
    let rig = rig("gone-pack");
    let v = rig.core.get_settings();
    assert_eq!(v["playing_pack"], "brown");
    assert_eq!(v["startup"]["unusable_pack"], "gone-pack");
    assert_eq!(
        rig.saved().pack_id,
        "gone-pack",
        "the file is not rewritten"
    );
}

#[test]
fn ac21_with_no_default_pack_either_the_built_in_click_plays() {
    let rig = rig_with("gone-pack", &[("blue", "full")], |_| ());
    let v = rig.core.get_settings();
    assert!(v["playing_pack"].is_null());
    assert_eq!(v["startup"]["unusable_pack"], "gone-pack");
    let click_ms = ms(&builtin_click());
    assert_eq!(rig.letter_ms(), click_ms);
}

// ---- AC22: removing a pack that is not playing ----

#[test]
fn ac22_removing_a_pack_that_is_not_playing_changes_only_the_folder_and_the_list() {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &src);
    rig.core.set_settings(&json!({"pack_id": "brown"})).unwrap();
    let bytes = fs::read(&rig.settings_path).unwrap();
    let revision = rig.revision();
    let told = rig.told();

    let v = rig.core.remove_pack("my-clicks").expect("removed");

    assert!(!rig.imported.join("my-clicks").exists());
    assert!(v["change"].is_null());
    let ids: Vec<&str> = v["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["brown", "blue"]);
    assert_eq!(rig.ids(), ["brown", "blue"]);
    assert_eq!(fs::read(&rig.settings_path).unwrap(), bytes);
    assert_eq!(rig.revision(), revision);
    assert_eq!(rig.told(), told);
    assert_brown_plays(&rig, "brown still plays");
    assert_eq!(rig.core.get_settings()["playing_pack"], "brown");
    // Nothing is left beside the others, not even a temporary folder.
    assert!(fs::read_dir(&rig.imported).unwrap().next().is_none());
}

#[test]
fn ac22_the_source_folder_is_not_touched_by_a_removal() {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    let before = snapshot(&src);
    import_ok(&rig, &src);
    rig.core.remove_pack("my-clicks").unwrap();
    assert_eq!(snapshot(&src), before);
}

#[test]
fn ac22_edge_removing_a_pack_that_is_only_the_saved_pack_id_resets_it() {
    // The saved imported pack no longer loads: its pack.json names a file
    // that is gone. Start-up falls back to brown and leaves the file as it
    // was; removing the pack then resets `pack_id`.
    let rig = rig_with("broken", &[("brown", "default-only")], |imported| {
        let dir = imported.join("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("pack.json"), json_default(&["missing.wav"])).unwrap();
    });
    assert_eq!(rig.core.get_settings()["playing_pack"], "brown");
    assert_eq!(rig.ids(), ["brown", "broken"]);
    let revision = rig.revision();

    let v = rig.core.remove_pack("broken").expect("removed");

    assert!(!v["change"].is_null());
    assert_eq!(rig.saved().pack_id, "brown");
    assert_eq!(rig.revision(), revision + 1);
    assert!(!rig.imported.join("broken").exists());
}

// ---- AC23: removing the playing pack ----

#[test]
fn ac23_removing_the_playing_pack_plays_the_default_pack_and_saves_it() {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &src);
    assert_eq!(rig.letter_ms(), 33);
    let revision = rig.revision();
    let told = rig.told();

    let v = rig.core.remove_pack("my-clicks").expect("removed");

    assert_eq!(v["change"]["playing_pack"], "brown");
    assert_eq!(v["change"]["settings"]["pack_id"], "brown");
    assert_eq!(v["change"]["revision"], revision + 1);
    let ids: Vec<&str> = v["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&"my-clicks"));
    assert_eq!(rig.saved().pack_id, "brown");
    assert_eq!(rig.revision(), revision + 1);
    assert_eq!(rig.told(), told + 1);
    assert_brown_plays(&rig, "the default pack plays from the next key");
    assert!(!rig.imported.join("my-clicks").exists());
}

#[test]
fn ac23_when_the_default_pack_cannot_load_the_built_in_click_plays() {
    let rig = rig_with("blue", &[("blue", "full")], |_| ());
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &src);
    let v = rig.core.remove_pack("my-clicks").expect("removed");
    assert!(v["change"]["playing_pack"].is_null());
    assert_eq!(rig.letter_ms(), ms(&builtin_click()));
    assert!(rig.core.get_settings()["playing_pack"].is_null());
}

// ---- AC24: refused removals ----

#[test]
fn ac24_a_bundled_an_unknown_or_malformed_id_is_refused_and_nothing_changes() {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    import_ok(&rig, &src);
    // Things a malformed id could reach: the imported folder's parent and
    // the bundled folder.
    let parent = rig.imported.parent().unwrap().to_path_buf();
    fs::create_dir_all(parent.join("x")).unwrap();
    fs::write(parent.join("x/keep.txt"), "keep").unwrap();

    let data_before = snapshot(&parent);
    let bundled_before = snapshot(&rig.bundled);
    let revision = rig.revision();
    let told = rig.told();

    for id in [
        "brown",
        "blue",
        "nope",
        "../x",
        "/etc",
        "",
        "..",
        ".",
        "my-clicks/..",
        "x/../my-clicks",
    ] {
        assert!(rig.core.remove_pack(id).is_err(), "{id:?} must be refused");
    }

    assert_eq!(snapshot(&parent), data_before);
    assert_eq!(snapshot(&rig.bundled), bundled_before);
    assert_eq!(rig.revision(), revision);
    assert_eq!(rig.told(), told);
    assert!(rig.imported.join("my-clicks/pack.json").is_file());
}

#[test]
fn ac24_the_refusal_messages_name_the_cause() {
    let rig = rig("brown");
    assert_eq!(
        rig.core.remove_pack("brown").err().unwrap(),
        "\"brown\" is a bundled pack and cannot be removed"
    );
    assert_eq!(
        rig.core.remove_pack("nope").err().unwrap(),
        "unknown sound pack \"nope\""
    );
}

// ---- AC26: the key path never waits for an import or a removal ----

/// A subscriber runs under the settings lock, which `apply` holds, and the
/// library lock is taken before it. Blocking in it holds both locks inside
/// an import (or a removal) for as long as the test likes.
fn key_path_runs_while_a_change_is_held(removing: bool) {
    let rig = rig("brown");
    let (_t, src) = source_with_lengths("my-clicks", 33, None);
    if removing {
        import_ok(&rig, &src);
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let entered_tx = Mutex::new(entered_tx);
    let counter = Arc::clone(&calls);
    let armed = AtomicUsize::new(0);
    // The first call is the one `subscribe` makes now; the next is the change.
    rig.core.subscribe(Box::new(move |_: &Value| {
        if armed.fetch_add(1, Ordering::SeqCst) >= 1 {
            counter.fetch_add(1, Ordering::SeqCst);
            let _ = entered_tx.lock().unwrap().send(());
            let _ = release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20));
        }
    }));

    let core = Arc::clone(&rig.core);
    let src_path = src.clone();
    let change = std::thread::spawn(move || {
        if removing {
            core.remove_pack("my-clicks").map(|_| ())
        } else {
            core.import_pack(&src_path).map(|_| ())
        }
    });
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the change reached its subscribers");

    // The change is now held mid-way, under its locks. Keys must play.
    let player = Arc::clone(&rig.player);
    let sink = Arc::clone(&rig.sink);
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let keys = std::thread::spawn(move || {
        for _ in 0..50 {
            player.handle(down(KeyCode::KeyA));
            player.handle(down(KeyCode::Space));
        }
        let _ = done_tx.send(());
    });
    let finished = done_rx.recv_timeout(Duration::from_secs(5));
    let _ = release_tx.send(());
    change.join().unwrap().expect("the change itself succeeds");
    if finished.is_ok() {
        keys.join().unwrap();
    }
    assert!(finished.is_ok(), "key events waited for the held change");
    assert_eq!(sink.count(), 100);
}

#[test]
fn ac26_key_events_are_played_while_an_import_is_held_in_its_change() {
    key_path_runs_while_a_change_is_held(false);
}

#[test]
fn ac26_key_events_are_played_while_a_removal_is_held_in_its_change() {
    key_path_runs_while_a_change_is_held(true);
}

// ---- Review follow-ups ----

#[test]
fn ac24_a_bundled_pack_with_an_unreadable_pack_json_is_still_refused_as_bundled() {
    let rig = rig("brown");
    fs::write(rig.bundled.join("brown/pack.json"), "{ nope").unwrap();
    copy_dir(&fixture_pack("extra-field"), &rig.imported.join("brown"));
    assert!(!rig.ids().contains(&"brown".to_string()));
    let before = snapshot(&rig.imported);
    assert_eq!(
        rig.core.remove_pack("brown").expect_err("refused"),
        "\"brown\" is a bundled pack and cannot be removed"
    );
    assert_eq!(snapshot(&rig.imported), before);
}

#[cfg(unix)]
#[test]
fn ac17_a_double_quote_in_the_folder_name_is_written_as_it_is() {
    let rig = rig("brown");
    let tmp = TempDir::new("src");
    let src = tmp.path().join("My \"best\" pack");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("notes.txt"), "x").unwrap();
    let err = rig.core.import_pack(&src).expect_err("refused");
    assert_eq!(
        err,
        "Could not import \"My \"best\" pack\": no audio files found (WAV, OGG, MP3 or FLAC)"
    );
}
