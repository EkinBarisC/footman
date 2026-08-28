//! The Run Action actually running things (DESIGN.md §3).
//!
//! The interesting claim is not "a process started" but "the command line the
//! user wrote was interpreted by a command interpreter". The example in §3 is
//! `pnpm dev`, and `pnpm` on Windows is a `.cmd` shim: starting the executable
//! directly would work for `notepad` and fail for most of what anyone would
//! actually bind. So these tests use shell syntax that no plain `CreateProcess`
//! could honour, and check for its effect on disk.

#![cfg(windows)]

use std::time::{Duration, Instant};

use footman::windows::{CREATE_NEW_CONSOLE, CREATE_NO_WINDOW, creation_flags, run_command};

/// Commands are started and let go of, so the effect has to be waited for.
fn wait_for(path: &std::path::Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn a_command_runs_and_has_its_effect() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let marker = scratch.path().join("ran.txt");

    run_command(
        &format!("echo footman> {}", marker.display()),
        // Hidden: a test must not flash a console window at whoever is running
        // it, and hiding is the mode a bound command will normally use.
        false,
    )
    .expect("the command must start");

    assert!(wait_for(&marker), "the command did not run");
}

/// Redirection, `&&` and quoting only mean anything to a command interpreter.
/// This is the test that fails if `cmd` is ever taken out of the path, or if
/// the command line is passed as an ordinary quoted argument.
#[test]
fn shell_syntax_is_honoured_rather_than_escaped() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let first = scratch.path().join("first.txt");
    let second = scratch.path().join("second.txt");

    run_command(
        &format!(
            "echo one> {} && echo two> {}",
            first.display(),
            second.display()
        ),
        false,
    )
    .expect("the command must start");

    assert!(
        wait_for(&second),
        "the second half of the command did not run"
    );
    assert!(first.exists(), "the first half of the command did not run");
}

/// A command that does not exist is the shell's problem, not Footman's: `cmd`
/// starts, reports its own error, and exits. Footman does not wait for it, so
/// there is nothing to report and nothing to fail.
#[test]
fn a_command_that_cannot_be_found_is_not_footmans_failure() {
    assert!(run_command("footman-no-such-command", false).is_ok());
}

/// A visible command must get a console of its *own*.
///
/// Left to itself, a child console process inherits its parent's console, so a
/// command bound with `show_window = true` printed into whatever terminal
/// Footman happened to be started from — and printed into a fresh console of
/// its own when Footman was started from none. The window a user asked for has
/// to be asked for explicitly, or the behaviour depends on how Footman was
/// launched.
#[test]
fn a_visible_command_is_given_a_console_of_its_own() {
    assert_eq!(creation_flags(true), CREATE_NEW_CONSOLE);
}

#[test]
fn a_hidden_command_is_given_no_console_at_all() {
    assert_eq!(creation_flags(false), CREATE_NO_WINDOW);
}
