//! The per-user Scheduled Task that starts Footman at logon (ADR-0005).
//!
//! The task is registered by handing `schtasks` an XML definition, which makes
//! the definition a string Footman builds — and a string is testable without a
//! scheduler. What is asserted here is every setting the decision was made for:
//! a `Run` registry value would have been one line of code, and it was rejected
//! because of what it *cannot* say. If the XML does not say those things, the
//! decision has been paid for and not bought.

#![cfg(windows)]

use std::path::Path;

use footman::windows::task;

fn definition() -> String {
    task::definition(
        Path::new(r"C:\Users\ada\AppData\Local\Footman\footman.exe"),
        "LAPTOP\\ada",
    )
}

#[test]
fn the_task_runs_the_executable_it_was_given() {
    assert!(definition().contains(r"C:\Users\ada\AppData\Local\Footman\footman.exe"));
}

/// A user name may contain an ampersand, and a path may sit under one. Written
/// raw it would make the definition not be XML at all, and `schtasks` would
/// refuse the whole thing for a reason pointing nowhere near the user's name.
#[test]
fn a_path_that_is_not_xml_safe_is_escaped() {
    let xml = task::definition(Path::new(r"C:\Users\a&b\footman.exe"), "LAPTOP\\a&b");

    assert!(xml.contains("a&amp;b"), "the ampersand must be escaped");
    assert!(!xml.contains(r"a&b"), "and not left raw anywhere");
}

#[test]
fn it_starts_at_logon() {
    assert!(definition().contains("<LogonTrigger>"));
}

/// The complaint that started Footman was a launcher that did not reliably come
/// back. A task that gives up after one failed start would reproduce it.
#[test]
fn it_is_restarted_if_it_fails() {
    let xml = definition();
    assert!(xml.contains("<RestartOnFailure>"));
    assert!(xml.contains("<Count>"));
}

/// Scheduled tasks are killed after three days by default. Footman is meant to
/// still be there in a month.
#[test]
fn it_is_not_given_a_time_limit() {
    assert!(definition().contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
}

/// A laptop is the normal case, and the scheduler's defaults are written for a
/// desktop: on battery it would neither start Footman nor let it keep running.
#[test]
fn a_machine_on_battery_is_not_a_reason_to_stay_shut() {
    let xml = definition();
    assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
    assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
}

/// Two Footmen means two hooks on one keyboard, each suppressing keys the other
/// is waiting for.
#[test]
fn a_second_copy_is_not_started_over_the_first() {
    assert!(definition().contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
}

/// Unelevated, as DESIGN.md §8 requires: a keyboard hook must not run with
/// administrator privilege, and anything Footman launches inherits what it has.
#[test]
fn it_runs_with_no_more_privilege_than_the_user_has() {
    assert!(definition().contains("<RunLevel>LeastPrivilege</RunLevel>"));
}

/// The tray icon cannot be created before the shell is ready to hold it.
#[test]
fn it_waits_a_little_after_logon() {
    assert!(definition().contains("<Delay>"));
}

#[test]
fn the_task_is_registered_by_name_and_definition() {
    let arguments = task::create(Path::new(r"C:\Temp\footman.xml"));

    assert_eq!(
        arguments,
        vec![
            "/Create",
            "/TN",
            "Footman",
            "/XML",
            r"C:\Temp\footman.xml",
            "/F"
        ],
        "/F so that registering twice replaces rather than fails"
    );
}

#[test]
fn the_task_is_removed_by_name() {
    assert_eq!(task::delete(), vec!["/Delete", "/TN", "Footman", "/F"]);
}

#[test]
fn the_task_is_asked_after_by_name() {
    assert_eq!(task::query(), vec!["/Query", "/TN", "Footman"]);
}
