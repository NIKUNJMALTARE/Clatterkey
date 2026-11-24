//! Tests for the decisions of feature 11 (launch at login) that need no
//! `Core`: the start-up comparison, what makes a start a login start, and
//! whether the window is shown (AC14, AC15, AC17, AC18, AC19). The system's
//! registration is a fake; nothing here touches the real one (AC21).
//!
//! Interfaces assumed from the plan:
//!
//! ```ignore
//! pub trait LoginItem: Send + Sync { enable, disable, is_enabled, tracks_system }
//! pub struct Inert;
//! pub fn reconcile(setting: bool, item: &dyn LoginItem, err: &ErrorOut) -> bool
//! pub const HIDDEN_ARG: &str;
//! pub fn is_login_start<S: AsRef<OsStr>>(args: &[S]) -> bool
//! pub fn show_at_start(login_start: bool, permission: PermissionState, tray_present: bool) -> bool
//! pub fn show_on_second_launch<S: AsRef<OsStr>>(args: &[S]) -> bool
//! pub fn without_hidden(args: Vec<OsString>) -> Vec<OsString>
//! ```

use std::ffi::OsString;

use clickclak_lib::autostart::{
    HIDDEN_ARG, Inert, LoginItem, is_login_start, reconcile, show_at_start, show_on_second_launch,
    without_hidden,
};
use clickclak_lib::permission::PermissionState;

mod common;
use common::*;
mod support;
use support::*;

fn lines(buffer: &Captured) -> Vec<String> {
    written(buffer).lines().map(str::to_string).collect()
}

// AC14: the four cases

#[test]
fn ac14_on_with_no_registration_becomes_off_and_nothing_is_made() {
    let item = FakeItem::new(false);
    let (buffer, err) = fake_err();
    assert!(!reconcile(true, &*item, &err));
    assert!(!item.calls().contains(&"enable"), "no registration is made");
    assert!(!item.calls().contains(&"disable"));
    assert_eq!(written(&buffer), "", "no notice and no line");
}

#[test]
fn ac14_on_with_a_registration_is_refreshed_once_and_stays_on() {
    let item = FakeItem::new(true);
    let (buffer, err) = fake_err();
    assert!(reconcile(true, &*item, &err));
    let enables = item.calls().iter().filter(|c| **c == "enable").count();
    assert_eq!(enables, 1);
    assert!(!item.calls().contains(&"disable"));
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac14_off_with_a_registration_removes_it_and_stays_off() {
    let item = FakeItem::new(true);
    let (buffer, err) = fake_err();
    assert!(!reconcile(false, &*item, &err));
    let disables = item.calls().iter().filter(|c| **c == "disable").count();
    assert_eq!(disables, 1);
    assert!(!item.calls().contains(&"enable"));
    assert_eq!(written(&buffer), "");
}

#[test]
fn ac14_off_with_none_changes_nothing() {
    let item = FakeItem::new(false);
    let (buffer, err) = fake_err();
    assert!(!reconcile(false, &*item, &err));
    assert!(!item.calls().contains(&"enable"));
    assert!(!item.calls().contains(&"disable"));
    assert_eq!(written(&buffer), "");
}

// AC15: failures keep the setting and write one `autostart:` line

#[test]
fn ac15_a_system_that_cannot_be_asked_leaves_the_setting_with_one_line() {
    for setting in [true, false] {
        let item = FakeItem::new(true);
        item.cannot_be_asked("registry is locked");
        let (buffer, err) = fake_err();
        assert_eq!(reconcile(setting, &*item, &err), setting);
        assert!(!item.calls().contains(&"enable"));
        assert!(!item.calls().contains(&"disable"));
        let out = lines(&buffer);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].starts_with("autostart:"), "{out:?}");
        assert!(out[0].contains("registry is locked"), "{out:?}");
    }
}

#[test]
fn ac15_a_failed_refresh_keeps_the_setting_on_with_one_line() {
    let item = FakeItem::new(true);
    item.fail_enable("disk is full");
    let (buffer, err) = fake_err();
    assert!(reconcile(true, &*item, &err));
    let out = lines(&buffer);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(out[0].starts_with("autostart:"), "{out:?}");
    assert!(out[0].contains("disk is full"), "{out:?}");
}

#[test]
fn ac15_a_failed_removal_keeps_the_setting_off_with_one_line() {
    let item = FakeItem::new(true);
    item.fail_disable("access denied");
    let (buffer, err) = fake_err();
    assert!(!reconcile(false, &*item, &err));
    let out = lines(&buffer);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(out[0].starts_with("autostart:"), "{out:?}");
    assert!(out[0].contains("access denied"), "{out:?}");
}

// AC19: the inert registration

#[test]
fn ac19_inert_never_fails_and_reports_none() {
    let item = Inert;
    assert_eq!(item.enable(), Ok(()));
    assert_eq!(item.disable(), Ok(()));
    assert_eq!(item.is_enabled(), Ok(false));
    assert!(!item.tracks_system());
}

#[test]
fn ac19_reconcile_over_inert_leaves_the_setting_as_it_is_and_writes_nothing() {
    for setting in [true, false] {
        let (buffer, err) = fake_err();
        assert_eq!(reconcile(setting, &Inert, &err), setting);
        assert_eq!(written(&buffer), "");
    }
}

#[test]
fn ac19_reconcile_does_not_ask_a_registration_that_is_not_real() {
    let item = FakeItem::untracked();
    let (buffer, err) = fake_err();
    assert!(reconcile(true, &*item, &err));
    assert!(!reconcile(false, &*item, &err));
    assert!(
        item.calls().is_empty(),
        "nothing called: {:?}",
        item.calls()
    );
    assert_eq!(written(&buffer), "");
}

// AC17: whether the window is shown at start

#[test]
fn ac17_a_login_start_with_a_tray_icon_stays_hidden_when_nothing_is_missing() {
    for p in [PermissionState::Granted, PermissionState::NotNeeded] {
        assert!(!show_at_start(true, p, true), "{p:?}");
    }
}

#[test]
fn ac17_a_login_start_with_the_permission_missing_shows_the_window() {
    for p in [PermissionState::Needed, PermissionState::RestartNeeded] {
        assert!(show_at_start(true, p, true), "{p:?}");
    }
}

#[test]
fn ac17_a_login_start_with_no_tray_icon_shows_the_window_in_every_case() {
    for p in ALL {
        assert!(show_at_start(true, p, false), "{p:?}");
    }
}

#[test]
fn ac17_a_start_that_is_not_a_login_start_always_shows_the_window() {
    for p in ALL {
        for tray in [true, false] {
            assert!(show_at_start(false, p, tray), "{p:?} tray {tray}");
        }
    }
}

const ALL: [PermissionState; 4] = [
    PermissionState::NotNeeded,
    PermissionState::Needed,
    PermissionState::Granted,
    PermissionState::RestartNeeded,
];

// AC18: what makes a login start

#[test]
fn ac18_the_argument_is_hidden() {
    assert_eq!(HIDDEN_ARG, "--hidden");
}

#[test]
fn ac18_a_start_is_a_login_start_exactly_when_the_arguments_carry_it() {
    assert!(is_login_start(&["app", "--hidden"]));
    assert!(is_login_start(&["app", "other", "--hidden"]));
    assert!(is_login_start(&["/Apps/Keys", "--hidden", "other"]));
    assert!(!is_login_start(&["app"]));
    assert!(!is_login_start::<&str>(&[]));
    assert!(!is_login_start(&["app", "other"]));
}

#[test]
fn ac18_edge_the_program_path_itself_is_not_an_argument() {
    assert!(!is_login_start(&["--hidden"]));
    assert!(!is_login_start(&["--hidden", "other"]));
}

#[test]
fn ac18_edge_near_misses_are_not_a_login_start() {
    assert!(!is_login_start(&["app", "--hidden=1"]));
    assert!(!is_login_start(&["app", "-hidden"]));
    assert!(!is_login_start(&["app", "--Hidden"]));
    assert!(!is_login_start(&["app", "hidden"]));
    assert!(!is_login_start(&["app", "--hidden "]));
}

#[test]
fn ac18_arguments_may_be_os_strings() {
    let args: Vec<OsString> = vec!["app".into(), "--hidden".into()];
    assert!(is_login_start(&args));
    assert!(!show_on_second_launch(&args));
}

#[test]
fn ac18_a_second_launch_shows_the_window_unless_it_is_a_login_start() {
    assert!(show_on_second_launch(&["app"]));
    assert!(show_on_second_launch(&["app", "other"]));
    assert!(show_on_second_launch(&["--hidden"]));
    assert!(!show_on_second_launch(&["app", "--hidden"]));
}

#[test]
fn ac18_restart_now_drops_the_hidden_argument_and_keeps_the_rest() {
    let args: Vec<OsString> = vec!["app".into(), "--hidden".into(), "--other".into()];
    assert_eq!(
        without_hidden(args),
        vec![OsString::from("app"), OsString::from("--other")]
    );
    let none: Vec<OsString> = vec!["app".into(), "x".into()];
    assert_eq!(without_hidden(none.clone()), none);
    let twice: Vec<OsString> = vec!["app".into(), "--hidden".into(), "--hidden".into()];
    assert_eq!(without_hidden(twice), vec![OsString::from("app")]);
}
