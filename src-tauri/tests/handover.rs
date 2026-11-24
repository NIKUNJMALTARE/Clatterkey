//! Tests for the hand-over from the old app name (feature 13: AC6 to AC17)
//! over temporary folders, a captured error writer and the recording fake
//! registration. Nothing touches a real login registration.
//!
//! Interfaces assumed from the plan:
//!
//! ```ignore
//! // clickclak_lib::handover
//! pub enum Copied { Nothing, Done, Failed }   // matched with `matches!`
//! pub fn old_folder(new_folder: &Path) -> Option<PathBuf>
//! pub fn copy_files(old: &Path, new: &Path, err: &ErrorOut) -> Copied
//! pub fn login(copied: Copied, setting_on: bool, registers: bool,
//!              old: &dyn LoginItem, new: &dyn LoginItem, err: &ErrorOut)
//! ```
//!
//! `copy_files` and `login` are also assumed to need nothing but their
//! arguments (no Tauri). `Copied` is only matched, never compared or
//! printed, so it needs no derives.
//!
//! This file and the hand-over module are the only places outside
//! `docs/features/` that may name the old folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clickclak_lib::audio::OutputStatus;
use clickclak_lib::autostart::{Inert, LoginItem, reconcile};
use clickclak_lib::commands::{start_core, start_core_after};
use clickclak_lib::handover::{Copied, copy_files, login, old_folder};
use clickclak_lib::packs::Library;
use clickclak_lib::permission::{Permission, PermissionState};
use clickclak_lib::settings::{self, LoadProblem, Settings};

mod common;
use common::*;
mod support;
use support::*;

const OLD_NAME: &str = "com.mechanicalkeysounds.app";
const NEW_NAME: &str = "com.clickclak.app";

/// An old and a new app folder side by side in a fresh temporary folder.
/// Neither exists until a test makes it.
struct Folders {
    _root: TempDir,
    old: PathBuf,
    new: PathBuf,
}

fn folders() -> Folders {
    let root = TempDir::new("handover");
    let old = root.path().join(OLD_NAME);
    let new = root.path().join(NEW_NAME);
    Folders {
        _root: root,
        old,
        new,
    }
}

/// An old folder with a settings file naming the imported pack `mypack`
/// and that pack (a copy of the `full` fixture, so it is a usable pack).
fn old_with_everything(f: &Folders) {
    fs::create_dir_all(&f.old).unwrap();
    fs::write(f.old.join("settings.json"), file("mypack")).unwrap();
    copy_dir(&fixture_pack("full"), &f.old.join("packs").join("mypack"));
}

fn lines(buffer: &Captured) -> Vec<String> {
    written(buffer).lines().map(str::to_string).collect()
}

fn is_copied(copied: &Copied) -> bool {
    matches!(copied, Copied::Done)
}

fn is_nothing(copied: &Copied) -> bool {
    matches!(copied, Copied::Nothing)
}

fn is_failed(copied: &Copied) -> bool {
    matches!(copied, Copied::Failed)
}

fn defaults() -> Settings {
    Settings::default()
}

// ---- old_folder ----

#[test]
fn old_folder_is_the_sibling_named_by_the_old_identifier() {
    let got = old_folder(Path::new("/data/support").join(NEW_NAME).as_path());
    assert_eq!(got, Some(Path::new("/data/support").join(OLD_NAME)));
}

#[test]
fn old_folder_is_none_when_the_new_folder_has_no_parent() {
    assert_eq!(old_folder(Path::new("/")), None);
}

// ---- AC6 ----

#[test]
fn ac6_copies_settings_and_packs_and_leaves_the_old_folder_byte_for_byte() {
    let f = folders();
    old_with_everything(&f);
    let before = snapshot(&f.old);
    let (buffer, err) = fake_err();

    let result = copy_files(&f.old, &f.new, &err);

    assert!(is_copied(&result), "expected Copied::Done");
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        fs::read(f.old.join("settings.json")).unwrap()
    );
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.old.join("packs")),
        "an equal copy of the pack"
    );
    assert_eq!(snapshot(&f.new), snapshot(&f.old), "and nothing else left");
    assert_eq!(snapshot(&f.old), before, "the old folder is as it was");
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac6_edge_an_existing_empty_new_folder_is_filled() {
    let f = folders();
    old_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    let (_buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(snapshot(&f.new), snapshot(&f.old));
}

#[test]
fn ac6_edge_a_leftover_hidden_temporary_folder_does_not_stop_the_copy() {
    let f = folders();
    old_with_everything(&f);
    let leftover = f.new.join(".handover-tmp");
    fs::create_dir_all(&leftover).unwrap();
    fs::write(leftover.join("settings.json"), "half").unwrap();
    let (_buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        fs::read(f.old.join("settings.json")).unwrap(),
        "the copy, not the half-written leftover"
    );
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.old.join("packs"))
    );
}

#[test]
fn ac6_edge_a_hidden_folder_inside_the_old_packs_is_copied_as_it_is() {
    let f = folders();
    old_with_everything(&f);
    let stopped = f.old.join("packs").join(".import-tmp");
    fs::create_dir_all(&stopped).unwrap();
    fs::write(stopped.join("a.wav"), b"x").unwrap();
    let (_buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.old.join("packs"))
    );
}

#[test]
fn ac6_edge_a_second_call_after_done_does_nothing() {
    let f = folders();
    old_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    let after_first = snapshot(&f.new);
    fs::write(f.new.join("settings.json"), "changed under the new name").unwrap();
    let changed = snapshot(&f.new);
    assert_ne!(after_first, changed);
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(snapshot(&f.new), changed, "never overwritten by old values");
}

// ---- AC7 ----

#[test]
fn ac7_start_up_over_the_copy_uses_the_old_settings_and_plays_the_imported_pack() {
    let f = folders();
    old_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));

    let bundled = packs_folder("bundled", &[("brown", "default-only")]);
    let library = Library::new(
        Some(bundled.path().to_path_buf()),
        Some(f.new.join("packs")),
    );
    let (_buffer, err) = fake_err();
    let (core, _live) = start_core(
        library,
        Some(f.new.join("settings.json")),
        Box::new(|| OutputStatus::Playing),
        Arc::new(Permission::new(PermissionState::NotNeeded)),
        Arc::new(Inert),
        &err,
    );

    let got = core.get_settings();
    assert_eq!(got["settings"]["pack_id"], "mypack");
    assert_eq!(got["settings"]["volume"], 0.8);
    assert_eq!(got["settings"]["pitch_variation"], 0.2);
    assert_eq!(got["settings"]["key_up_sounds"], false);
    assert_eq!(got["playing_pack"], "mypack", "the saved pack is playing");
    let listed = core.list_packs();
    let mine = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "mypack")
        .expect("the imported pack is in the list");
    assert_eq!(mine["imported"], true);
}

// ---- AC8 ----

#[test]
fn ac8_a_new_folder_with_a_settings_file_is_left_alone() {
    let f = folders();
    old_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join("settings.json"), "new settings").unwrap();
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert!(!f.new.join("packs").exists());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac8_a_new_folder_with_a_packs_folder_is_left_alone() {
    let f = folders();
    old_with_everything(&f);
    fs::create_dir_all(f.new.join("packs")).unwrap();
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert!(!f.new.join("settings.json").exists());
    assert_eq!(written(&buffer), "");
}

#[cfg(unix)]
#[test]
fn ac8_the_old_folder_is_not_read_when_the_new_one_has_its_files() {
    use std::os::unix::fs::PermissionsExt;
    let f = folders();
    old_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join("settings.json"), "new settings").unwrap();
    // A folder that cannot be listed: reading it would fail loudly.
    fs::set_permissions(&f.old, fs::Permissions::from_mode(0o000)).unwrap();
    let (buffer, err) = fake_err();
    let result = copy_files(&f.old, &f.new, &err);
    fs::set_permissions(&f.old, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(is_nothing(&result));
    assert_eq!(written(&buffer), "", "nothing was read, so nothing failed");
}

// ---- AC9 ----

#[test]
fn ac9_no_old_folder_creates_and_changes_nothing_and_writes_nothing() {
    let f = folders();
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert!(!f.new.exists(), "the new folder is not even created");
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac9_no_old_folder_leaves_an_existing_new_folder_as_it_was() {
    let f = folders();
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join("other.txt"), "x").unwrap();
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac9_edge_an_old_path_that_is_a_file_counts_as_no_old_folder() {
    let f = folders();
    fs::write(&f.old, "not a folder").unwrap();
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert!(!f.new.exists());
    assert_eq!(written(&buffer), "");
}

// ---- AC10 ----

#[test]
fn ac10_an_old_folder_with_only_settings_copies_only_the_settings() {
    let f = folders();
    fs::create_dir_all(&f.old).unwrap();
    fs::write(f.old.join("settings.json"), file("full")).unwrap();
    let (buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        fs::read(f.old.join("settings.json")).unwrap()
    );
    assert!(!f.new.join("packs").exists());
    assert_eq!(files_under(&f.new), ["settings.json"]);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac10_an_old_folder_with_only_packs_copies_only_the_packs_and_settings_default() {
    let f = folders();
    fs::create_dir_all(&f.old).unwrap();
    copy_dir(&fixture_pack("full"), &f.old.join("packs").join("mypack"));
    let (buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert!(!f.new.join("settings.json").exists());
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.old.join("packs"))
    );
    assert_eq!(written(&buffer), "");

    // The usual start-up then creates the defaults.
    let (_buffer, err) = fake_err();
    let loaded = settings::load_or_create_report(&f.new.join("settings.json"), &err);
    assert_eq!(loaded.settings, defaults());
}

#[test]
fn ac10_edge_an_old_folder_with_neither_file_copies_nothing_and_does_not_fail() {
    let f = folders();
    fs::create_dir_all(&f.old).unwrap();
    fs::write(f.old.join("notes.txt"), "x").unwrap();
    let (buffer, err) = fake_err();
    let result = copy_files(&f.old, &f.new, &err);
    assert!(!is_failed(&result));
    assert!(!f.new.join("settings.json").exists());
    assert!(!f.new.join("packs").exists());
    assert!(!f.new.join("notes.txt").exists(), "only the two are copied");
    assert_eq!(written(&buffer), "");
}

// ---- AC11 ----

#[cfg(unix)]
fn unreadable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
    // Running as root, for one, can still read it: the test cannot run.
    fs::read(path).is_err()
}

#[cfg(unix)]
fn readable_again(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
}

#[cfg(unix)]
#[test]
fn ac11_an_unreadable_file_in_the_old_packs_leaves_nothing_in_the_new_folder() {
    let f = folders();
    old_with_everything(&f);
    let bad = f
        .old
        .join("packs")
        .join("mypack")
        .join("down")
        .join("a1.wav");
    if !unreadable(&bad) {
        readable_again(&bad);
        return;
    }
    let (buffer, err) = fake_err();
    let result = copy_files(&f.old, &f.new, &err);
    readable_again(&bad);

    assert!(is_failed(&result), "the result reports the failure");
    assert!(!f.new.join("settings.json").exists(), "no settings.json");
    assert!(!f.new.join("packs").exists(), "no packs folder");
    assert!(
        files_under(&f.new).is_empty(),
        "no half-copied file or temporary folder content: {:?}",
        files_under(&f.new)
    );
    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "exactly one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
}

#[cfg(unix)]
#[test]
fn ac11_start_up_goes_on_with_the_defaults_and_the_copy_is_not_tried_again() {
    let f = folders();
    old_with_everything(&f);
    let bad = f.old.join("settings.json");
    if !unreadable(&bad) {
        readable_again(&bad);
        return;
    }
    let (buffer, err) = fake_err();
    let result = copy_files(&f.old, &f.new, &err);
    assert!(is_failed(&result));
    assert_eq!(lines(&buffer).len(), 1);
    assert!(lines(&buffer)[0].starts_with("migrate:"));
    assert!(!f.new.join("packs").exists(), "no packs from the attempt");

    // Start-up: the usual first run, which writes a settings file.
    let (_buffer, err) = fake_err();
    let loaded = settings::load_or_create_report(&f.new.join("settings.json"), &err);
    assert_eq!(loaded.settings, defaults());
    assert!(f.new.join("settings.json").exists());

    // The copy is not tried again.
    readable_again(&bad);
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert_eq!(written(&buffer), "");
}

// ---- AC12 ----

#[test]
fn ac12_a_settings_file_that_is_not_an_object_is_copied_as_it_is_and_read_with_defaults() {
    let f = folders();
    fs::create_dir_all(&f.old).unwrap();
    fs::write(f.old.join("settings.json"), "[1, 2, 3]").unwrap();
    let (buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(fs::read(f.new.join("settings.json")).unwrap(), b"[1, 2, 3]");
    assert_eq!(written(&buffer), "", "the copy itself is no failure");

    let (_buffer, err) = fake_err();
    let loaded = settings::load_or_create_report(&f.new.join("settings.json"), &err);
    assert_eq!(loaded.settings, defaults());
    assert_eq!(loaded.problem, Some(LoadProblem::Unreadable));
}

#[test]
fn ac12_a_settings_file_with_a_field_out_of_range_is_copied_and_that_field_defaults() {
    let f = folders();
    fs::create_dir_all(&f.old).unwrap();
    let text = file("full").replace("\"pitch_variation\": 0.2", "\"pitch_variation\": 0.9");
    assert!(text.contains("0.9"));
    fs::write(f.old.join("settings.json"), &text).unwrap();
    let (_buffer, err) = fake_err();
    assert!(is_copied(&copy_files(&f.old, &f.new, &err)));
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        text.as_bytes()
    );

    let (_buffer, err) = fake_err();
    let loaded = settings::load_or_create_report(&f.new.join("settings.json"), &err);
    assert_eq!(loaded.settings.pitch_variation, defaults().pitch_variation);
    assert_eq!(loaded.settings.pack_id, "full", "the rest is as saved");
    assert_eq!(loaded.settings.volume, 0.8);
    assert!(!loaded.settings.key_up_sounds);
    assert_eq!(
        loaded.problem,
        Some(LoadProblem::Replaced(vec!["pitch_variation"]))
    );
}

// ---- login (AC13 to AC17) ----

fn assert_in_force(item: &Arc<FakeItem>, want: bool) {
    assert_eq!(
        *item.in_force.lock().unwrap(),
        Ok(want),
        "calls: {:?}",
        item.calls()
    );
}

#[test]
fn ac13_old_in_force_and_setting_on_moves_the_registration_and_the_setting_stays_on() {
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    login(Copied::Done, true, true, &*old, &*new, &err);

    assert!(old.calls().contains(&"disable"), "the old one is removed");
    assert_in_force(&old, false);
    assert!(new.calls().contains(&"enable"));
    assert_in_force(&new, true);
    assert_eq!(written(&buffer), "");
    assert!(
        reconcile(true, &*new, &err),
        "the start-up comparison keeps it on"
    );
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac13_edge_no_new_registration_is_made_when_nothing_was_copied() {
    // The settings in force are then the new app's own, not the old ones.
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (_buffer, err) = fake_err();
    login(Copied::Nothing, true, true, &*old, &*new, &err);
    assert!(old.calls().contains(&"disable"));
    assert!(!new.calls().contains(&"enable"));
    assert_in_force(&new, false);
}

#[test]
fn ac13_edge_no_new_registration_is_made_when_the_copy_failed() {
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (_buffer, err) = fake_err();
    login(Copied::Failed, false, true, &*old, &*new, &err);
    assert!(old.calls().contains(&"disable"));
    assert!(!new.calls().contains(&"enable"));
}

#[test]
fn ac14_an_absent_old_registration_makes_no_new_one_and_the_setting_turns_off() {
    let old = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    login(Copied::Done, true, true, &*old, &*new, &err);

    assert!(!new.calls().contains(&"enable"), "no new one is made");
    assert_in_force(&new, false);
    assert!(
        old.calls().contains(&"disable"),
        "removing what is not there is tried and is no error"
    );
    assert_eq!(written(&buffer), "");
    assert!(!reconcile(true, &*new, &err), "the setting turns off");
}

#[test]
fn ac14_an_old_registration_switched_off_in_the_system_is_removed_and_not_replaced() {
    // The system reports a registration that is present but switched off
    // as not in force.
    let old = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    login(Copied::Done, true, true, &*old, &*new, &err);
    assert!(old.calls().contains(&"is_enabled"), "it is asked");
    assert!(old.calls().contains(&"disable"), "and removed");
    assert!(!new.calls().contains(&"enable"));
    assert!(!reconcile(true, &*new, &err));
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac15_a_setting_that_is_off_removes_the_old_registration_and_makes_no_new_one() {
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    login(Copied::Done, false, true, &*old, &*new, &err);
    assert!(old.calls().contains(&"disable"));
    assert_in_force(&old, false);
    assert!(!new.calls().contains(&"enable"));
    assert_in_force(&new, false);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac15_with_no_old_folder_an_old_registration_is_still_removed_at_a_later_start() {
    let f = folders();
    let (buffer, err) = fake_err();
    let copied = copy_files(&f.old, &f.new, &err);
    assert!(is_nothing(&copied));
    let old = FakeItem::new(true);
    let new = FakeItem::new(true);
    login(copied, true, true, &*old, &*new, &err);
    assert!(old.calls().contains(&"disable"));
    assert_in_force(&old, false);
    assert_in_force(&new, true);
    assert!(
        !new.calls().contains(&"disable") && !new.calls().contains(&"enable"),
        "the new registration is left to the usual comparison: {:?}",
        new.calls()
    );
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac16_failing_to_remove_the_old_registration_writes_one_migrate_line_and_goes_on() {
    let old = FakeItem::new(true);
    old.fail_disable("permission denied (os error 13)");
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    let dir = TempDir::new("login-settings");
    let path = dir.path().join("settings.json");
    fs::write(&path, file("full")).unwrap();
    let before = fs::read(&path).unwrap();

    login(Copied::Done, true, true, &*old, &*new, &err);

    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert!(got[0].contains("permission denied"), "says why: {}", got[0]);
    assert_eq!(fs::read(&path).unwrap(), before, "settings file unchanged");
}

#[test]
fn ac16_failing_to_make_the_new_registration_writes_one_migrate_line_and_goes_on() {
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    new.fail_enable("no space left (os error 28)");
    let (buffer, err) = fake_err();

    login(Copied::Done, true, true, &*old, &*new, &err);

    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert!(got[0].contains("no space left"), "{}", got[0]);
    // The usual comparison then decides: nothing in force, so it is off.
    let (_buffer, err) = fake_err();
    assert!(!reconcile(true, &*new, &err));
}

#[test]
fn ac16_edge_an_old_registration_that_cannot_be_read_still_has_its_removal_tried() {
    let old = FakeItem::new(true);
    old.cannot_be_asked("registry unavailable");
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    login(Copied::Done, true, true, &*old, &*new, &err);

    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert!(old.calls().contains(&"disable"), "removal is still tried");
    assert!(
        !new.calls().contains(&"enable"),
        "no new registration without knowing the old was in force"
    );
}

#[test]
fn ac17_a_development_build_asks_nothing_of_either_registration() {
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    login(Copied::Done, true, false, &*old, &*new, &err);
    assert!(old.calls().is_empty(), "old: {:?}", old.calls());
    assert!(new.calls().is_empty(), "new: {:?}", new.calls());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac17_a_development_build_still_copies_the_files_as_in_ac6() {
    let f = folders();
    old_with_everything(&f);
    let (_buffer, err) = fake_err();
    let copied = copy_files(&f.old, &f.new, &err);
    assert!(is_copied(&copied));
    assert_eq!(snapshot(&f.new), snapshot(&f.old));
    let old = FakeItem::new(true);
    let new = FakeItem::new(false);
    login(copied, true, false, &*old, &*new, &err);
    assert!(old.calls().is_empty() && new.calls().is_empty());
}

// ---- AC11 without Unix permissions ----

#[test]
fn ac11_a_plain_file_where_the_temporary_folder_goes_fails_the_copy_on_every_platform() {
    let f = folders();
    old_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join(".handover-tmp"), "in the way").unwrap();
    let before = snapshot(&f.old);
    let (buffer, err) = fake_err();

    let result = copy_files(&f.old, &f.new, &err);

    assert!(is_failed(&result), "the result reports the failure");
    assert!(!f.new.join("settings.json").exists(), "no settings.json");
    assert!(!f.new.join("packs").exists(), "no packs folder");
    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "exactly one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert_eq!(snapshot(&f.old), before, "the old folder is as it was");
}

// ---- The start-up order: copy, read, old registration, compare ----

/// Starts the core over a settings file saying `launch_at_login: on`, with
/// `hook` run between reading the settings and the comparison. Returns the
/// value the hook was given, the core's setting and the file's setting.
fn start_with_hook(on: bool, item: &Arc<FakeItem>, hook: impl FnOnce(bool)) -> (bool, bool, bool) {
    let dir = TempDir::new("order");
    let path = dir.path().join("settings.json");
    let text = if on {
        file("full")
    } else {
        file("full").replace("\"launch_at_login\": true", "\"launch_at_login\": false")
    };
    fs::write(&path, text).unwrap();
    let bundled = packs_folder("bundled", &[("full", "full")]);
    let (_buffer, err) = fake_err();
    let given = std::cell::Cell::new(None);
    let (core, _live) = start_core_after(
        Library::bundled_only(bundled.path().to_path_buf()),
        Some(path.clone()),
        Box::new(|| OutputStatus::Playing),
        Arc::new(Permission::new(PermissionState::NotNeeded)),
        item.clone(),
        &err,
        |read| {
            given.set(Some(read));
            hook(read);
        },
    );
    let in_core = core.get_settings()["settings"]["launch_at_login"]
        .as_bool()
        .unwrap();
    let in_file = settings::read(&path).unwrap().settings.launch_at_login;
    (given.get().expect("the hook ran"), in_core, in_file)
}

#[test]
fn ac13_the_hook_runs_before_the_comparison_so_a_moved_registration_keeps_the_setting_on() {
    let item = FakeItem::new(false);
    let enabler = item.clone();
    let (given, in_core, in_file) = start_with_hook(true, &item, |read| {
        assert!(read);
        enabler.enable().unwrap();
    });
    assert!(given, "the hook is given the setting as read");
    assert!(in_core, "still on in the core");
    assert!(in_file, "still on in the file");
}

#[test]
fn ac14_with_a_hook_that_does_nothing_the_comparison_turns_the_setting_off() {
    let item = FakeItem::new(false);
    let (given, in_core, in_file) = start_with_hook(true, &item, |_| ());
    assert!(given);
    assert!(!in_core, "the usual comparison turns it off");
    assert!(!in_file);
}

#[test]
fn the_hook_is_given_false_when_the_file_says_off() {
    let item = FakeItem::new(false);
    let (given, in_core, _) = start_with_hook(false, &item, |_| ());
    assert!(!given);
    assert!(!in_core);
}
