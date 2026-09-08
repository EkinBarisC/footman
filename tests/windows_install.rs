//! Where Footman lives, and how it leaves (DESIGN.md §11, ADR-0005).
//!
//! Footman ships as one portable executable and installs itself, so it has to
//! know two things it cannot ask anyone: where its copy belongs, and whether it
//! is already that copy. Both are decisions about paths, and paths can be
//! decided without a disk.
//!
//! Leaving is the harder half. A running executable cannot delete itself, and
//! Uninstall is offered from the settings window of the very copy being
//! uninstalled — so the removal has to outlive the process asking for it.

#![cfg(windows)]

use std::path::{Path, PathBuf};

use footman::windows::install;

fn local() -> PathBuf {
    PathBuf::from(r"C:\Users\ada\AppData\Local")
}

/// A folder of its own, not loose in `Local`: Uninstall removes the folder, and
/// what it removes must never be able to contain anything else.
#[test]
fn footman_lives_in_a_folder_of_its_own() {
    assert_eq!(install::home_in(&local()), local().join("Footman"));
}

#[test]
fn the_installed_copy_keeps_the_name_it_shipped_with() {
    assert_eq!(
        install::copy_in(&install::home_in(&local())),
        local().join("Footman").join("footman.exe")
    );
}

#[test]
fn the_copy_in_that_folder_is_the_installed_one() {
    let home = install::home_in(&local());

    assert!(install::is_installed(&install::copy_in(&home), &home));
}

/// Windows does not distinguish paths by case, and the scheduler, the shell and
/// the user all spell them differently. Reading the same file as two files
/// would have Footman install itself over itself for ever.
#[test]
fn the_same_path_spelled_differently_is_the_same_path() {
    let home = install::home_in(&local());
    let shouted = PathBuf::from(r"C:\USERS\ADA\APPDATA\LOCAL\FOOTMAN\FOOTMAN.EXE");

    assert!(install::is_installed(&shouted, &home));
}

#[test]
fn a_copy_running_from_anywhere_else_is_not_the_installed_one() {
    let home = install::home_in(&local());

    assert!(!install::is_installed(
        Path::new(r"D:\Downloads\footman.exe"),
        &home
    ));
    assert!(
        !install::is_installed(
            Path::new(r"C:\Users\ada\AppData\Local\Footman\old\footman.exe"),
            &home
        ),
        "a copy below the folder is not the copy in it"
    );
}

/// Windows refuses to write over a running executable but will happily rename
/// one, which is the whole of how an install over a running Footman works.
#[test]
fn the_copy_being_replaced_is_moved_aside_not_written_over() {
    let home = install::home_in(&local());

    assert_ne!(install::displaced_in(&home), install::copy_in(&home));
}

/// Inside the home, because Uninstall removes the home and nothing else: a
/// displaced copy anywhere above it would be the residue §11 promises not to
/// leave, sitting somewhere nobody would go looking.
#[test]
fn the_copy_moved_aside_stays_inside_the_home() {
    let home = install::home_in(&local());

    assert!(install::displaced_in(&home).starts_with(&home));
}

/// It keeps a name of its own, and specifically not the one the Task points
/// at. A displaced copy that still read as the installed one would have
/// Footman decline to install over itself for ever after.
#[test]
fn the_copy_moved_aside_is_not_the_installed_one() {
    let home = install::home_in(&local());

    assert!(!install::is_installed(&install::displaced_in(&home), &home));
}

/// The removal has to survive the process that asks for it, so it is handed to
/// something else with instructions to wait. Deleting first and waiting after
/// would delete nothing at all.
#[test]
fn leaving_waits_before_it_deletes() {
    let farewell = install::farewell(&install::home_in(&local()));

    let waited = farewell.find("ping").expect("something has to wait");
    let deleted = farewell.find("rmdir").expect("and then delete");
    assert!(waited < deleted, "in that order: {farewell}");
}

/// Quoted, because the path runs through `AppData\Local` and a user's name may
/// contain a space. Unquoted, `cmd` would take the first word and delete
/// whatever that turned out to name.
#[test]
fn the_folder_being_removed_is_quoted() {
    let home = install::home_in(&local());

    assert!(install::farewell(&home).contains(&format!("\"{}\"", home.display())));
}
