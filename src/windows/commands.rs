//! The Run Action: execute a command line (DESIGN.md §3).

use std::os::windows::process::CommandExt;
use std::process::Command;

/// The Run Action: execute a command line (DESIGN.md §3).
///
/// Through `cmd`, not directly: the example in §3 is `pnpm dev`, and `pnpm` on
/// Windows is a `.cmd` shim that only a command interpreter can start. Running
/// the executable directly would work for `notepad` and fail for most of what
/// anyone would actually bind.
/// Give the command a console window of its own.
///
/// Not the default. A child console process inherits its parent's console, so
/// without this a visible command prints into whatever terminal Footman was
/// started from — and once Footman runs from the tray with no console of its
/// own, it would behave differently again. Both halves are stated explicitly so
/// neither depends on how Footman was launched.
pub const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Give it no console at all.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn creation_flags(show_window: bool) -> u32 {
    if show_window {
        CREATE_NEW_CONSOLE
    } else {
        CREATE_NO_WINDOW
    }
}

pub fn run(command: &str, show_window: bool) -> Result<(), String> {
    let mut child = Command::new("cmd");
    child.creation_flags(creation_flags(show_window));
    // Verbatim rather than as an argument: `cmd /C` has its own quoting rules,
    // and letting Rust quote the command first would mangle anything with
    // spaces or `&` in it.
    child.raw_arg(format!("/C {command}"));

    // Started and let go of. Footman does not wait for it, report its exit
    // status, or hold its output: a Binding is a shortcut, not a job runner.
    child
        .spawn()
        .map(drop)
        .map_err(|error| format!("could not run {command}: {error}"))
}
