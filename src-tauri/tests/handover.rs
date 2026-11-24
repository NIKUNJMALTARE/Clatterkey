//! Tests for the hand-over from the earlier app names (feature 14: AC6 to
//! AC18) over temporary folders, a captured error writer and the recording
//! fake registration. Nothing touches a real login registration.
//!
//! Interfaces assumed from the plan:
//!
//! ```ignore
//! // clatterkey_lib::handover
//! pub struct Earlier { pub identifier: &'static str, pub app_name: &'static str }
//! pub const EARLIER: [Earlier; 2]   // ClickClak, then Mechanical Key Sounds
//! pub fn earlier_folders(new_folder: &Path) -> Vec<PathBuf>
//! pub enum Copied { Nothing, Done(usize), Failed }   // matched with `matches!`
//! pub fn copy_files(earlier: &[PathBuf], new: &Path, err: &ErrorOut) -> Copied
//! pub fn login(copied: Copied, setting_on: bool, registers: bool,
//!              earlier: &[&dyn LoginItem], new: &dyn LoginItem, err: &ErrorOut)
//! ```
//!
//! `Done(i)` is the position, in the list given, of the folder copied from.
//! `copy_files` and `login` need nothing but their arguments (no Tauri).
//! `Copied` is only matched, never compared or printed.
//!
//! This file and the hand-over module are the only places outside
//! `docs/features/` that may name the earlier folders.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clatterkey_lib::audio::OutputStatus;
use clatterkey_lib::autostart::{Inert, LoginItem, reconcile};
use clatterkey_lib::commands::{start_core, start_core_after};
use clatterkey_lib::handover::{Copied, EARLIER, copy_files, earlier_folders, login};
use clatterkey_lib::packs::Library;
use clatterkey_lib::permission::{Permission, PermissionState};
use clatterkey_lib::settings::{self, LoadProblem, Settings};

mod common;
use common::*;
mod support;
use support::*;

const CLICK_NAME: &str = "com.clickclak.app";
const MKS_NAME: &str = "com.mechanicalkeysounds.app";
const NEW_NAME: &str = "com.clatterkey.app";

/// The three app folders side by side in a fresh temporary folder. None
/// exists until a test makes it.
struct Folders {
    _root: TempDir,
    click: PathBuf,
    mks: PathBuf,
    new: PathBuf,
}

fn folders() -> Folders {
    let root = TempDir::new("handover");
    let click = root.path().join(CLICK_NAME);
    let mks = root.path().join(MKS_NAME);
    let new = root.path().join(NEW_NAME);
    Folders {
        _root: root,
        click,
        mks,
        new,
    }
}

impl Folders {
    /// The earlier folders in the order the app gives them.
    fn earlier(&self) -> Vec<PathBuf> {
        vec![self.click.clone(), self.mks.clone()]
    }
}

/// A folder with a settings file naming the imported pack `pack_id` and
/// that pack (a copy of the fixture `fixture`, a usable pack).
fn with_everything(dir: &Path, settings_text: &str, pack_id: &str, fixture: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("settings.json"), settings_text).unwrap();
    copy_dir(&fixture_pack(fixture), &dir.join("packs").join(pack_id));
}

/// A folder holding what the ClickClak folder would: pack `mypack`.
fn click_with_everything(f: &Folders) {
    with_everything(&f.click, &file("mypack"), "mypack", "full");
}

/// A folder holding what the Mechanical Key Sounds folder would, with
/// values different from the ClickClak one.
fn mks_with_everything(f: &Folders) {
    let text = file("mksonly").replace("\"volume\": 0.8", "\"volume\": 0.3");
    with_everything(&f.mks, &text, "mksonly", "default-only");
}

fn lines(buffer: &Captured) -> Vec<String> {
    written(buffer).lines().map(str::to_string).collect()
}

fn done_from(copied: &Copied, position: usize) -> bool {
    matches!(copied, Copied::Done(i) if *i == position)
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

// ---- EARLIER and earlier_folders ----

#[test]
fn earlier_lists_clickclak_then_mechanical_key_sounds() {
    assert_eq!(EARLIER.len(), 2);
    assert_eq!(EARLIER[0].identifier, CLICK_NAME);
    assert_eq!(EARLIER[0].app_name, "ClickClak");
    assert_eq!(EARLIER[1].identifier, MKS_NAME);
    assert_eq!(EARLIER[1].app_name, "Mechanical Key Sounds");
}

#[test]
fn earlier_folders_gives_the_two_siblings_in_order() {
    let new = Path::new("/data/support").join(NEW_NAME);
    assert_eq!(
        earlier_folders(&new),
        vec![
            Path::new("/data/support").join(CLICK_NAME),
            Path::new("/data/support").join(MKS_NAME),
        ]
    );
}

#[test]
fn earlier_folders_is_empty_when_the_new_folder_has_no_parent() {
    assert!(earlier_folders(Path::new("/")).is_empty());
}

// ---- AC6 ----

#[test]
fn ac6_copies_settings_and_packs_from_clickclak_and_leaves_it_byte_for_byte() {
    let f = folders();
    click_with_everything(&f);
    let before = snapshot(&f.click);
    let (buffer, err) = fake_err();

    let result = copy_files(&f.earlier(), &f.new, &err);

    assert!(done_from(&result, 0), "expected Done(0)");
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        fs::read(f.click.join("settings.json")).unwrap()
    );
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.click.join("packs")),
        "an equal copy of the pack"
    );
    assert_eq!(
        snapshot(&f.new),
        snapshot(&f.click),
        "and nothing else left"
    );
    assert_eq!(
        snapshot(&f.click),
        before,
        "the ClickClak folder is as it was"
    );
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac6_the_folders_are_found_by_the_names_the_app_builds() {
    let f = folders();
    click_with_everything(&f);
    let (_buffer, err) = fake_err();
    let result = copy_files(&earlier_folders(&f.new), &f.new, &err);
    assert!(done_from(&result, 0));
    assert_eq!(snapshot(&f.new), snapshot(&f.click));
}

#[test]
fn ac6_edge_an_existing_empty_new_folder_is_filled() {
    let f = folders();
    click_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert_eq!(snapshot(&f.new), snapshot(&f.click));
}

#[test]
fn ac6_edge_a_leftover_hidden_temporary_folder_does_not_stop_the_copy() {
    let f = folders();
    click_with_everything(&f);
    let leftover = f.new.join(".handover-tmp");
    fs::create_dir_all(&leftover).unwrap();
    fs::write(leftover.join("settings.json"), "half").unwrap();
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        fs::read(f.click.join("settings.json")).unwrap(),
        "the copy, not the half-written leftover"
    );
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.click.join("packs"))
    );
}

#[test]
fn ac6_edge_a_hidden_folder_inside_the_earlier_packs_is_copied_as_it_is() {
    let f = folders();
    click_with_everything(&f);
    let stopped = f.click.join("packs").join(".import-tmp");
    fs::create_dir_all(&stopped).unwrap();
    fs::write(stopped.join("a.wav"), b"x").unwrap();
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.click.join("packs"))
    );
}

#[test]
fn ac6_edge_a_second_call_after_done_does_nothing() {
    let f = folders();
    click_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    let after_first = snapshot(&f.new);
    fs::write(f.new.join("settings.json"), "changed under the new name").unwrap();
    let changed = snapshot(&f.new);
    assert_ne!(after_first, changed);
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert_eq!(
        snapshot(&f.new),
        changed,
        "never overwritten by earlier values"
    );
}

// ---- AC7 ----

#[test]
fn ac7_copies_from_mechanical_key_sounds_when_it_is_the_only_one() {
    let f = folders();
    mks_with_everything(&f);
    let before = snapshot(&f.mks);
    let (buffer, err) = fake_err();

    let result = copy_files(&f.earlier(), &f.new, &err);

    assert!(done_from(&result, 1), "expected Done(1)");
    assert_eq!(snapshot(&f.new), snapshot(&f.mks));
    assert_eq!(snapshot(&f.mks), before);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac7_a_clickclak_folder_with_neither_file_is_passed_over() {
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    fs::write(f.click.join("notes.txt"), "x").unwrap();
    mks_with_everything(&f);
    let (buffer, err) = fake_err();
    let result = copy_files(&f.earlier(), &f.new, &err);
    assert!(done_from(&result, 1));
    assert_eq!(snapshot(&f.new), snapshot(&f.mks));
    assert!(!f.new.join("notes.txt").exists());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac7_an_empty_clickclak_folder_is_passed_over() {
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    mks_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 1));
    assert_eq!(snapshot(&f.new), snapshot(&f.mks));
}

#[test]
fn ac7_a_clickclak_path_that_is_a_file_is_passed_over() {
    let f = folders();
    fs::write(&f.click, "not a folder").unwrap();
    mks_with_everything(&f);
    let (buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 1));
    assert_eq!(snapshot(&f.new), snapshot(&f.mks));
    assert_eq!(written(&buffer), "");
}

// ---- AC8 ----

#[test]
fn ac8_with_both_folders_the_new_one_equals_clickclak_and_nothing_is_merged() {
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    let click_before = snapshot(&f.click);
    let mks_before = snapshot(&f.mks);
    let (buffer, err) = fake_err();

    let result = copy_files(&f.earlier(), &f.new, &err);

    assert!(done_from(&result, 0));
    assert_eq!(
        snapshot(&f.new),
        click_before,
        "equals the ClickClak folder"
    );
    assert!(!f.new.join("packs").join("mksonly").exists());
    let text = fs::read_to_string(f.new.join("settings.json")).unwrap();
    assert!(!text.contains("mksonly") && !text.contains("0.3"));
    assert_eq!(snapshot(&f.click), click_before);
    assert_eq!(snapshot(&f.mks), mks_before);
    assert_eq!(written(&buffer), "");
}

// ---- AC9 ----

fn start_over_the_new_folder(f: &Folders) -> clatterkey_lib::commands::Core {
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
    core
}

#[test]
fn ac9_start_up_over_the_copy_from_clickclak_uses_its_settings_and_plays_its_pack() {
    let f = folders();
    click_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));

    let core = start_over_the_new_folder(&f);

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

#[test]
fn ac9_start_up_over_the_copy_from_mechanical_key_sounds_uses_its_settings_and_pack() {
    let f = folders();
    mks_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 1));

    let core = start_over_the_new_folder(&f);

    let got = core.get_settings();
    assert_eq!(got["settings"]["pack_id"], "mksonly");
    assert_eq!(got["settings"]["volume"], 0.3);
    assert_eq!(got["playing_pack"], "mksonly");
    let listed = core.list_packs();
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "mksonly" && p["imported"] == true)
    );
}

// ---- AC10 ----

#[test]
fn ac10_a_new_folder_with_a_settings_file_is_left_alone() {
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join("settings.json"), "new settings").unwrap();
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert!(!f.new.join("packs").exists());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac10_a_new_folder_with_a_packs_folder_is_left_alone() {
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    fs::create_dir_all(f.new.join("packs")).unwrap();
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert!(!f.new.join("settings.json").exists());
    assert_eq!(written(&buffer), "");
}

#[cfg(unix)]
#[test]
fn ac10_neither_earlier_folder_is_read_when_the_new_one_has_its_files() {
    use std::os::unix::fs::PermissionsExt;
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join("settings.json"), "new settings").unwrap();
    // Folders that cannot be listed: reading them would fail loudly.
    for dir in [&f.click, &f.mks] {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o000)).unwrap();
    }
    let (buffer, err) = fake_err();
    let result = copy_files(&f.earlier(), &f.new, &err);
    for dir in [&f.click, &f.mks] {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(is_nothing(&result));
    assert_eq!(written(&buffer), "", "nothing was read, so nothing failed");
}

// ---- AC11 ----

#[test]
fn ac11_no_earlier_folder_creates_and_changes_nothing_and_writes_nothing() {
    let f = folders();
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert!(!f.new.exists(), "the new folder is not even created");
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac11_no_earlier_folder_leaves_an_existing_new_folder_as_it_was() {
    let f = folders();
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join("other.txt"), "x").unwrap();
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac11_edge_an_empty_list_of_earlier_folders_is_nothing_and_writes_nothing() {
    let f = folders();
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&[], &f.new, &err)));
    assert!(!f.new.exists());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac11_edge_both_earlier_paths_being_files_count_as_no_earlier_folder() {
    let f = folders();
    fs::write(&f.click, "not a folder").unwrap();
    fs::write(&f.mks, "not a folder").unwrap();
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert!(!f.new.exists());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac11_edge_earlier_folders_with_neither_file_copy_nothing_and_do_not_fail() {
    let f = folders();
    for dir in [&f.click, &f.mks] {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("notes.txt"), "x").unwrap();
    }
    let (buffer, err) = fake_err();
    let result = copy_files(&f.earlier(), &f.new, &err);
    assert!(is_nothing(&result));
    assert!(!f.new.exists());
    assert_eq!(written(&buffer), "");
}

// ---- AC12 ----

#[test]
fn ac12_a_source_with_only_settings_copies_only_the_settings() {
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    fs::write(f.click.join("settings.json"), file("full")).unwrap();
    let (buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        fs::read(f.click.join("settings.json")).unwrap()
    );
    assert!(!f.new.join("packs").exists());
    assert_eq!(files_under(&f.new), ["settings.json"]);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac12_a_source_with_only_packs_copies_only_the_packs_and_settings_default() {
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    copy_dir(&fixture_pack("full"), &f.click.join("packs").join("mypack"));
    let (buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert!(!f.new.join("settings.json").exists());
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.click.join("packs"))
    );
    assert_eq!(written(&buffer), "");

    // The usual start-up then creates the defaults.
    let (_buffer, err) = fake_err();
    let loaded = settings::load_or_create_report(&f.new.join("settings.json"), &err);
    assert_eq!(loaded.settings, defaults());
}

#[test]
fn ac12_a_clickclak_folder_with_only_settings_is_the_source_and_packs_are_not_taken_from_the_other()
{
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    fs::write(f.click.join("settings.json"), file("full")).unwrap();
    mks_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert_eq!(files_under(&f.new), ["settings.json"]);
    assert_eq!(
        fs::read(f.new.join("settings.json")).unwrap(),
        file("full").as_bytes()
    );
    assert!(!f.new.join("packs").exists());
}

#[test]
fn ac12_a_clickclak_folder_with_only_packs_is_the_source_and_settings_are_not_taken_from_the_other()
{
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    copy_dir(&fixture_pack("full"), &f.click.join("packs").join("mypack"));
    mks_with_everything(&f);
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
    assert!(!f.new.join("settings.json").exists());
    assert_eq!(
        snapshot(&f.new.join("packs")),
        snapshot(&f.click.join("packs"))
    );
    assert!(!f.new.join("packs").join("mksonly").exists());
}

#[test]
fn ac12_a_settings_file_that_is_not_an_object_is_copied_as_it_is_and_read_with_defaults() {
    let f = folders();
    fs::create_dir_all(&f.click).unwrap();
    fs::write(f.click.join("settings.json"), "[1, 2, 3]").unwrap();
    let (buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 0));
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
    fs::create_dir_all(&f.mks).unwrap();
    let text = file("full").replace("\"pitch_variation\": 0.2", "\"pitch_variation\": 0.9");
    assert!(text.contains("0.9"));
    fs::write(f.mks.join("settings.json"), &text).unwrap();
    let (_buffer, err) = fake_err();
    assert!(done_from(&copy_files(&f.earlier(), &f.new, &err), 1));
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

// ---- AC13 ----

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
fn ac13_an_unreadable_file_in_the_clickclak_packs_leaves_nothing_and_the_other_folder_is_not_tried()
{
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    let bad = f
        .click
        .join("packs")
        .join("mypack")
        .join("down")
        .join("a1.wav");
    if !unreadable(&bad) {
        readable_again(&bad);
        return;
    }
    let mks_before = snapshot(&f.mks);
    let (buffer, err) = fake_err();
    let result = copy_files(&f.earlier(), &f.new, &err);
    readable_again(&bad);

    assert!(is_failed(&result), "the result reports the failure");
    assert!(!f.new.join("settings.json").exists(), "no settings.json");
    assert!(!f.new.join("packs").exists(), "no packs folder");
    assert!(
        files_under(&f.new).is_empty(),
        "nothing half-copied and nothing from the other folder: {:?}",
        files_under(&f.new)
    );
    assert_eq!(snapshot(&f.mks), mks_before);
    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "exactly one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
}

#[cfg(unix)]
#[test]
fn ac13_start_up_goes_on_with_the_defaults_and_the_copy_is_not_tried_again() {
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    let bad = f.click.join("settings.json");
    if !unreadable(&bad) {
        readable_again(&bad);
        return;
    }
    let (buffer, err) = fake_err();
    let result = copy_files(&f.earlier(), &f.new, &err);
    assert!(is_failed(&result));
    assert_eq!(lines(&buffer).len(), 1);
    assert!(lines(&buffer)[0].starts_with("migrate:"));
    assert!(!f.new.join("packs").exists(), "no packs from the attempt");

    // Start-up: the usual first run, which writes a settings file.
    let (_buffer, err) = fake_err();
    let loaded = settings::load_or_create_report(&f.new.join("settings.json"), &err);
    assert_eq!(loaded.settings, defaults());
    assert!(f.new.join("settings.json").exists());

    // The copy is not tried again, from either folder.
    readable_again(&bad);
    let before = snapshot(&f.new);
    let (buffer, err) = fake_err();
    assert!(is_nothing(&copy_files(&f.earlier(), &f.new, &err)));
    assert_eq!(snapshot(&f.new), before);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac13_a_plain_file_where_the_temporary_folder_goes_fails_the_copy_on_every_platform() {
    let f = folders();
    click_with_everything(&f);
    mks_with_everything(&f);
    fs::create_dir_all(&f.new).unwrap();
    fs::write(f.new.join(".handover-tmp"), "in the way").unwrap();
    let click_before = snapshot(&f.click);
    let mks_before = snapshot(&f.mks);
    let (buffer, err) = fake_err();

    let result = copy_files(&f.earlier(), &f.new, &err);

    assert!(is_failed(&result), "the result reports the failure");
    assert!(!f.new.join("settings.json").exists(), "no settings.json");
    assert!(!f.new.join("packs").exists(), "no packs folder");
    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "exactly one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert_eq!(snapshot(&f.click), click_before);
    assert_eq!(snapshot(&f.mks), mks_before, "nothing was tried from it");
}

// ---- login (AC14 to AC18) ----

fn assert_in_force(item: &Arc<FakeItem>, want: bool) {
    assert_eq!(
        *item.in_force.lock().unwrap(),
        Ok(want),
        "calls: {:?}",
        item.calls()
    );
}

/// Runs `login` with the two earlier registrations in the app's order.
fn run_login(
    copied: Copied,
    on: bool,
    registers: bool,
    click: &Arc<FakeItem>,
    mks: &Arc<FakeItem>,
    new: &Arc<FakeItem>,
    err: &ErrOut,
) {
    let earlier: [&dyn LoginItem; 2] = [&**click, &**mks];
    login(copied, on, registers, &earlier, &**new, err);
}

#[test]
fn ac14_clickclak_copy_with_its_registration_in_force_moves_it_and_the_setting_stays_on() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);

    assert_in_force(&click, false);
    assert!(click.calls().contains(&"disable"));
    assert!(mks.calls().contains(&"disable"), "both are removed");
    assert_in_force(&mks, false);
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
fn ac14_with_the_names_exchanged_the_mechanical_key_sounds_copy_moves_its_registration() {
    let click = FakeItem::new(false);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    run_login(Copied::Done(1), true, true, &click, &mks, &new, &err);

    assert_in_force(&click, false);
    assert_in_force(&mks, false);
    assert!(click.calls().contains(&"disable") && mks.calls().contains(&"disable"));
    assert_in_force(&new, true);
    assert_eq!(written(&buffer), "");
    assert!(reconcile(true, &*new, &err));
}

#[test]
fn ac14_both_registrations_in_force_and_the_copy_from_the_second_registers_the_new_one() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (_buffer, err) = fake_err();
    run_login(Copied::Done(1), true, true, &click, &mks, &new, &err);
    assert_in_force(&click, false);
    assert_in_force(&mks, false);
    assert_in_force(&new, true);
}

#[test]
fn ac14_goes_through_start_core_after_so_the_real_comparison_keeps_the_setting_on() {
    for (position, click_on, mks_on) in [(0usize, true, false), (1usize, false, true)] {
        let click = FakeItem::new(click_on);
        let mks = FakeItem::new(mks_on);
        let new = FakeItem::new(false);
        let (c, m, n) = (click.clone(), mks.clone(), new.clone());
        let (given, in_core, in_file) = start_with_hook(true, &new, move |read| {
            let (_buffer, err) = fake_err();
            run_login(Copied::Done(position), read, true, &c, &m, &n, &err);
        });
        assert!(given);
        assert!(in_core, "still on in the core (copied from {position})");
        assert!(in_file, "still on in the file (copied from {position})");
        assert_in_force(&click, false);
        assert_in_force(&mks, false);
    }
}

#[test]
fn ac15_an_absent_registration_of_the_copied_name_makes_no_new_one_and_the_setting_turns_off() {
    let click = FakeItem::new(false);
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);

    assert!(!new.calls().contains(&"enable"), "no new one is made");
    assert_in_force(&new, false);
    assert!(
        click.calls().contains(&"disable") && mks.calls().contains(&"disable"),
        "removing what is not there is tried and is no error"
    );
    assert_eq!(written(&buffer), "");
    assert!(!reconcile(true, &*new, &err), "the setting turns off");
}

#[test]
fn ac15_the_other_names_registration_in_force_does_not_count() {
    // Copied from ClickClak, whose registration is absent; the Mechanical
    // Key Sounds one is in force. Both are removed and no new one is made.
    let click = FakeItem::new(false);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);
    assert_in_force(&mks, false);
    assert_in_force(&click, false);
    assert!(!new.calls().contains(&"enable"));
    assert_in_force(&new, false);
    assert_eq!(written(&buffer), "");
    assert!(!reconcile(true, &*new, &err));

    // And the same with the names exchanged.
    let click = FakeItem::new(true);
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (_buffer, err) = fake_err();
    run_login(Copied::Done(1), true, true, &click, &mks, &new, &err);
    assert_in_force(&click, false);
    assert!(!new.calls().contains(&"enable"));
    assert!(!reconcile(true, &*new, &err));
}

#[test]
fn ac15_goes_through_start_core_after_so_the_real_comparison_turns_the_setting_off() {
    // Copied from the second name, whose registration is absent, while the
    // first name's is in force.
    let click = FakeItem::new(true);
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (c, m, n) = (click.clone(), mks.clone(), new.clone());
    let (_given, in_core, in_file) = start_with_hook(true, &new, move |read| {
        let (_buffer, err) = fake_err();
        run_login(Copied::Done(1), read, true, &c, &m, &n, &err);
    });
    assert!(!in_core);
    assert!(!in_file);
    assert_in_force(&click, false);
}

#[test]
fn ac15_edge_done_with_only_registration_zero_in_force_for_position_one_removes_both_and_makes_none()
 {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    run_login(Copied::Done(1), true, true, &click, &mks, &new, &err);
    assert!(click.calls().contains(&"disable"));
    assert!(mks.calls().contains(&"disable"));
    assert!(!new.calls().contains(&"enable"));
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac15_edge_a_position_with_no_registration_counts_as_not_in_force() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    run_login(Copied::Done(7), true, true, &click, &mks, &new, &err);
    assert_in_force(&click, false);
    assert_in_force(&mks, false);
    assert!(!new.calls().contains(&"enable"));
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac16_a_setting_that_is_off_removes_both_registrations_and_makes_no_new_one() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    run_login(Copied::Done(0), false, true, &click, &mks, &new, &err);
    assert_in_force(&click, false);
    assert_in_force(&mks, false);
    assert!(!new.calls().contains(&"enable"));
    assert_in_force(&new, false);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac16_nothing_copied_removes_an_earlier_registration_of_either_name_and_makes_no_new_one() {
    for (click_on, mks_on) in [(true, false), (false, true), (true, true)] {
        let click = FakeItem::new(click_on);
        let mks = FakeItem::new(mks_on);
        let new = FakeItem::new(true);
        let (buffer, err) = fake_err();
        run_login(Copied::Nothing, true, true, &click, &mks, &new, &err);
        assert_in_force(&click, false);
        assert_in_force(&mks, false);
        assert!(
            !new.calls().contains(&"disable") && !new.calls().contains(&"enable"),
            "the new registration is left to the usual comparison: {:?}",
            new.calls()
        );
        assert_in_force(&new, true);
        assert_eq!(written(&buffer), "");
    }
}

#[test]
fn ac16_with_no_earlier_folder_a_registration_is_still_removed_at_a_later_start() {
    let f = folders();
    let (buffer, err) = fake_err();
    let copied = copy_files(&f.earlier(), &f.new, &err);
    assert!(is_nothing(&copied));
    let click = FakeItem::new(false);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(true);
    run_login(copied, true, true, &click, &mks, &new, &err);
    assert_in_force(&mks, false);
    assert_in_force(&new, true);
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac16_a_failed_copy_removes_registrations_and_makes_no_new_one() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (_buffer, err) = fake_err();
    run_login(Copied::Failed, false, true, &click, &mks, &new, &err);
    assert_in_force(&click, false);
    assert_in_force(&mks, false);
    assert!(!new.calls().contains(&"enable"));
}

#[test]
fn ac17_failing_to_remove_one_registration_writes_one_migrate_line_and_the_other_is_still_removed()
{
    for failing in [0usize, 1] {
        let click = FakeItem::new(true);
        let mks = FakeItem::new(true);
        let new = FakeItem::new(false);
        let bad = if failing == 0 { &click } else { &mks };
        bad.fail_disable("permission denied (os error 13)");
        let (buffer, err) = fake_err();
        let dir = TempDir::new("login-settings");
        let path = dir.path().join("settings.json");
        fs::write(&path, file("full")).unwrap();
        let before = fs::read(&path).unwrap();

        run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);

        let got = lines(&buffer);
        assert_eq!(got.len(), 1, "one line (failing {failing}): {got:?}");
        assert!(got[0].starts_with("migrate:"), "{}", got[0]);
        assert!(got[0].contains("permission denied"), "says why: {}", got[0]);
        let other = if failing == 0 { &mks } else { &click };
        assert!(
            other.calls().contains(&"disable"),
            "the other is still removed"
        );
        assert_in_force(other, false);
        assert_eq!(fs::read(&path).unwrap(), before, "settings file unchanged");
    }
}

#[test]
fn ac17_a_registration_that_cannot_be_read_and_cannot_be_removed_gives_two_lines_and_the_next_is_dealt_with()
 {
    let click = FakeItem::new(true);
    click.cannot_be_asked("registry unavailable");
    click.fail_disable("access denied");
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);

    let got = lines(&buffer);
    assert_eq!(got.len(), 2, "two lines: {got:?}");
    assert!(got.iter().all(|l| l.starts_with("migrate:")), "{got:?}");
    assert!(
        mks.calls().contains(&"disable"),
        "the second is still removed"
    );
    assert_in_force(&mks, false);
    assert!(
        !new.calls().contains(&"enable"),
        "an unreadable registration counts as not in force"
    );
}

#[test]
fn ac17_a_registration_that_cannot_be_read_still_has_its_removal_tried() {
    let click = FakeItem::new(true);
    click.cannot_be_asked("registry unavailable");
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();

    run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);

    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert!(click.calls().contains(&"disable"), "removal is still tried");
    assert!(
        mks.calls().contains(&"disable"),
        "and the next one is dealt with"
    );
    assert!(!new.calls().contains(&"enable"));
}

#[test]
fn ac17_failing_to_make_the_new_registration_writes_one_migrate_line_and_the_comparison_decides() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(false);
    let new = FakeItem::new(false);
    new.fail_enable("no space left (os error 28)");
    let (buffer, err) = fake_err();

    run_login(Copied::Done(0), true, true, &click, &mks, &new, &err);

    let got = lines(&buffer);
    assert_eq!(got.len(), 1, "one line: {got:?}");
    assert!(got[0].starts_with("migrate:"), "{}", got[0]);
    assert!(got[0].contains("no space left"), "{}", got[0]);
    // The usual comparison then decides: nothing in force, so it is off.
    let (_buffer, err) = fake_err();
    assert!(!reconcile(true, &*new, &err));
}

#[test]
fn ac18_a_development_build_asks_nothing_of_any_registration() {
    let click = FakeItem::new(true);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    let (buffer, err) = fake_err();
    run_login(Copied::Done(0), true, false, &click, &mks, &new, &err);
    assert!(click.calls().is_empty(), "click: {:?}", click.calls());
    assert!(mks.calls().is_empty(), "mks: {:?}", mks.calls());
    assert!(new.calls().is_empty(), "new: {:?}", new.calls());
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac18_a_development_build_still_copies_the_files_as_in_ac6() {
    let f = folders();
    click_with_everything(&f);
    let (_buffer, err) = fake_err();
    let copied = copy_files(&f.earlier(), &f.new, &err);
    assert!(done_from(&copied, 0));
    assert_eq!(snapshot(&f.new), snapshot(&f.click));
    let click = FakeItem::new(true);
    let mks = FakeItem::new(true);
    let new = FakeItem::new(false);
    run_login(copied, true, false, &click, &mks, &new, &err);
    assert!(click.calls().is_empty() && mks.calls().is_empty() && new.calls().is_empty());
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
fn ac14_the_hook_runs_before_the_comparison_so_a_moved_registration_keeps_the_setting_on() {
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
fn ac15_with_a_hook_that_does_nothing_the_comparison_turns_the_setting_off() {
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
