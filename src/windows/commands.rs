//! The Run Action: execute a command line (DESIGN.md §3).

use std::os::windows::process::CommandExt;
use std::process::Command;

/// The Run Action: execute a command line (DESIGN.md §3).
///
/// Through `cmd`, not directly: the example in §3 is `pnpm dev`, and `pnpm` on
/// Windows is a `.cmd` shim that only a command interpreter can start. Running
/// the executable directly would work for `notepad` and fail for most of what
/// anyone would actually bind.
pub fn run(command: &str, show_window: bool) -> Result<(), String> {
    /// Start the process without giving it a console window of its own.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = Command::new("cmd");
    // Verbatim rather than as an argument: `cmd /C` has its own quoting rules,
    // and letting Rust quote the command first would mangle anything with
    // spaces or `&` in it.
    child.raw_arg(format!("/C {command}"));
    if !show_window {
        child.creation_flags(CREATE_NO_WINDOW);
    }

    // Started and let go of. Footman does not wait for it, report its exit
    // status, or hold its output: a Binding is a shortcut, not a job runner.
    child
        .spawn()
        .map(drop)
        .map_err(|error| format!("could not run {command}: {error}"))
}
