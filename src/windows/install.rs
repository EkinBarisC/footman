//! Installing and uninstalling Footman (DESIGN.md §11, ADR-0005).
//!
//! Footman ships as one portable executable with no installer. It installs
//! itself because the Scheduled Task has to name an absolute path, and a path
//! into the user's Downloads folder is not one to build a logon trigger on.
//!
//! Uninstall leaves nothing: the task, the config directory, and the installed
//! copy. The last of those is the awkward one — Uninstall is offered from the
//! settings window of the very copy being removed, and a running executable
//! cannot delete itself — so that removal is handed to a process told to wait
//! until this one has gone.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::commands::CREATE_NO_WINDOW;
use super::task;

/// The folder Footman installs itself into, under `%LOCALAPPDATA%`.
///
/// A folder of its own rather than a loose executable: Uninstall removes the
/// folder, and what it removes must never be able to contain anything else.
pub fn home_in(local_app_data: &Path) -> PathBuf {
    local_app_data.join("Footman")
}

/// The installed copy inside a home.
pub fn copy_in(home: &Path) -> PathBuf {
    home.join("footman.exe")
}

/// Where the copy being replaced is moved to.
///
/// Windows will not let a running executable be written over, but it will let
/// one be renamed — the image stays mapped to the file wherever it goes. So
/// installing over a Footman that is running moves the old copy aside and
/// writes the new one into the name the Task points at.
///
/// Measured rather than assumed, on the reference machine: writing over the
/// running copy fails and renaming it succeeds, which is the whole trick. What
/// does *not* work is deleting it afterwards — that fails for as long as its
/// process lives — so the displaced copy stays until the next install, which
/// begins by clearing it. It sits inside the home for that reason: it is
/// litter with a lifetime, and Uninstall takes the home whole.
pub fn displaced_in(home: &Path) -> PathBuf {
    home.join("footman.exe.old")
}

/// Whether this executable is the installed copy.
///
/// Compared without case, because Windows does not distinguish paths by it and
/// the scheduler, the shell and the user all spell them differently. Reading
/// one file as two would have Footman install itself over itself for ever.
pub fn is_installed(executable: &Path, home: &Path) -> bool {
    let same = |a: &Path| a.display().to_string().to_lowercase();
    same(executable) == same(&copy_in(home))
}

/// Where this user's home is, on this machine.
pub fn home() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|local| home_in(Path::new(&local)))
        .ok_or_else(|| "Windows did not say where this user's local data lives".to_string())
}

/// What Footman was started from.
fn running() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|error| format!("could not find my own executable: {error}"))
}

/// Whether an installed copy exists at all.
pub fn installed() -> bool {
    home().map(|home| copy_in(&home).exists()).unwrap_or(false)
}

/// Puts a copy in the home and returns where it went.
///
/// Installing while already installed is a no-op: the file that is there is the
/// file that would be written, and nothing can copy over itself.
///
/// Installing over an *older* copy is the case worth having. It is the ordinary
/// one — the second time anybody upgrades — and until this it failed with
/// `Access is denied` whenever that older copy happened to be running, which
/// after logon it usually is. It is moved aside rather than written over, for
/// the reason `displaced_in` gives.
pub fn install() -> Result<PathBuf, String> {
    let home = home()?;
    let running = running()?;
    let copy = copy_in(&home);

    if is_installed(&running, &home) {
        return Ok(copy);
    }

    std::fs::create_dir_all(&home)
        .map_err(|error| format!("could not make {}: {error}", home.display()))?;

    let displaced = displaced_in(&home);
    if copy.exists() {
        displace(&copy, &displaced)?;
    }

    if let Err(error) = std::fs::copy(&running, &copy) {
        // Put back what was moved. The Task names `copy` and nothing else, so
        // a failed install that left that name empty would take autostart with
        // it — an old Footman is a great deal better than none.
        let _ = std::fs::rename(&displaced, &copy);
        return Err(format!(
            "could not copy myself to {}: {error}",
            copy.display()
        ));
    }

    // Succeeds when the displaced copy was not running — an upgrade over a
    // Footman that was merely installed rather than started. When it was
    // running this fails, measured, and the file waits for the next install.
    let _ = std::fs::remove_file(&displaced);

    Ok(copy)
}

/// Moves the copy being replaced out of the name the new one wants.
fn displace(copy: &Path, displaced: &Path) -> Result<(), String> {
    // The last install's displaced copy, which is normally still here: this is
    // the only thing that ever clears it, and it succeeds now because whatever
    // was running it has long since gone. If it has not, the rename below is
    // the one that says so — two live generations of Footman is a thing to
    // report rather than to work around.
    let _ = std::fs::remove_file(displaced);

    std::fs::rename(copy, displaced).map_err(|error| {
        format!(
            "could not move {} aside to make room: {error}",
            copy.display()
        )
    })
}

/// Turns autostart on or off, installing first if it has to.
///
/// Turning it on is what installs Footman. Nothing copies itself anywhere
/// behind the user's back: the portable executable stays portable until asked
/// to be otherwise.
pub fn set_autostart(on: bool) -> Result<(), String> {
    if !on {
        return task::unregister();
    }

    let copy = install()?;
    task::register(&copy)
}

/// Removes the task, the config directory and the installed copy.
///
/// The copy goes last and by other hands: see `farewell`.
pub fn uninstall(config: &Path) -> Result<(), String> {
    task::unregister()?;

    // The config *directory*, not the file: the file is the only thing in it,
    // and leaving the folder behind is exactly the residue §11 promises not to
    // leave.
    if let Some(directory) = config.parent()
        && directory.exists()
    {
        std::fs::remove_dir_all(directory)
            .map_err(|error| format!("could not remove {}: {error}", directory.display()))?;
    }

    let home = home()?;
    if home.exists() {
        depart(&home)?;
    }
    Ok(())
}

/// The command that removes the home once Footman is no longer in it.
///
/// A running executable cannot delete itself, so the deletion is handed to a
/// `cmd` that waits first. `ping` is the wait: `timeout` needs a console to
/// count in, and this one has none. Quoted, because the path runs through
/// `AppData\Local` and a user's name may contain a space — unquoted, `cmd`
/// would take the first word and delete whatever that named.
pub fn farewell(home: &Path) -> String {
    format!(
        "ping -n 4 127.0.0.1 > nul & rmdir /s /q \"{}\"",
        home.display()
    )
}

/// Starts the removal and returns; the process outlives this one.
fn depart(home: &Path) -> Result<(), String> {
    Command::new("cmd")
        .raw_arg(format!("/C {}", farewell(home)))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not arrange to remove {}: {error}", home.display()))
}
