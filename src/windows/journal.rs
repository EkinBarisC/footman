//! A file Footman writes down what went wrong in.
//!
//! Footman is a Windows-subsystem process started by the logon task, so it has
//! no console: everything it says on standard error goes nowhere. Before this
//! file existed, a panic took the tray and the keyboard hook down together and
//! left no trace of why — not on screen, not in the event log, since an
//! unwinding panic is an ordinary exit as far as Windows is concerned. "Never
//! silently dead" (DESIGN.md §7) needs somewhere to say it.
//!
//! It lives beside the config file, so Uninstall takes it with the rest.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;

use windows::Win32::System::SystemInformation::GetLocalTime;

/// Where the journal is being written, once `start` has been called.
static FILE: OnceLock<PathBuf> = OnceLock::new();

/// Past this size the journal is set aside as `footman.log.old` and started
/// afresh, so a machine that has been failing for months still has a file
/// someone can open.
const LIMIT: u64 = 256 * 1024;

/// The journal's path, given the config file's.
pub fn beside(config: &Path) -> PathBuf {
    config.with_file_name("footman.log")
}

/// Starts writing to `path`, and sends every panic there too, on any thread.
///
/// The panic hook this replaces still runs afterwards: when there is a console,
/// the panic is printed on it as it always was.
pub fn start(path: PathBuf) {
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > LIMIT) {
        let _ = fs::rename(&path, path.with_extension("log.old"));
    }
    if FILE.set(path).is_err() {
        return;
    }

    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let current = thread::current();
        note(format!(
            "panic on the {} thread: {info}",
            current.name().unwrap_or("unnamed")
        ));
        previous(info);
    }));
}

/// Adds a line to the journal, if there is one. Failing to write is not itself
/// worth failing over: the journal is a witness, not a participant.
pub fn note(line: impl AsRef<str>) {
    let Some(path) = FILE.get() else { return };
    // Opened for each line rather than held: lines are rare, and a handle held
    // open would stop Uninstall removing the directory it is in.
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{} {}", timestamp(), line.as_ref());
}

/// Local wall-clock time. What a person reading the file compares it with is
/// the clock in their taskbar and the times in Event Viewer.
fn timestamp() -> String {
    let now = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    )
}
