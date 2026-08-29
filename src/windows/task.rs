//! The per-user Scheduled Task that starts Footman at logon (ADR-0005).
//!
//! A `Run` registry value would have been one line. It was rejected for what it
//! cannot say: Windows may defer it by `StartupDelayInMSec`, the user can turn
//! it off from Task Manager's Startup tab without knowing what they turned off,
//! and nothing brings it back if it exits. Every one of those is a setting
//! below, which is the whole reason for the extra machinery — and the reason
//! `tests/windows_task.rs` asserts each of them by name rather than trusting
//! the definition to be right because it parses.
//!
//! Registration goes through `schtasks` with an XML definition rather than
//! through its flags, because the flags cannot express restart-on-failure or an
//! unlimited run time. The definition is therefore a string Footman builds,
//! which is a thing that can be tested without a scheduler.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::commands::CREATE_NO_WINDOW;

/// What the task is called in the scheduler, and so what Uninstall looks for.
pub const NAME: &str = "Footman";

/// How long after logon to start.
///
/// The tray icon cannot be created before the shell is ready to hold it, and a
/// user who has just logged in is not reaching for a Chord in the first few
/// seconds anyway.
const DELAY: &str = "PT15S";

/// The definition to register.
///
/// `user` is the account the task runs as and belongs to, in the form
/// `DOMAIN\name` — without it `schtasks` cannot tell whose logon to wait for.
pub fn definition(executable: &Path, user: &str) -> String {
    let executable = escape(&executable.display().to_string());
    let user = escape(user);

    // Written as one string rather than assembled from a tree: it is read far
    // more often than it is changed, and every element of it is an argument
    // about behaviour that the tests name.
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts Footman, the hyper key launcher, at logon.</Description>
    <URI>\{NAME}</URI>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      <Delay>{DELAY}</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <DisallowStartOnRemoteAppSession>false</DisallowStartOnRemoteAppSession>
    <UseUnifiedSchedulingEngine>true</UseUnifiedSchedulingEngine>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{executable}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// XML-escapes text going into the definition.
///
/// Not decoration: a Windows account name may contain an ampersand, and the
/// path to the executable runs through it. Left raw, the definition stops being
/// XML and `schtasks` refuses all of it for a reason that points nowhere near
/// the user's name.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Arguments that register the task, replacing any earlier one.
pub fn create(xml: &Path) -> Vec<String> {
    // `/F` so that registering twice replaces rather than fails: turning
    // autostart off and on again, or installing over an older copy, must not
    // depend on the user having removed the old task first.
    strings(&[
        "/Create",
        "/TN",
        NAME,
        "/XML",
        &xml.display().to_string(),
        "/F",
    ])
}

/// Arguments that remove the task.
pub fn delete() -> Vec<String> {
    strings(&["/Delete", "/TN", NAME, "/F"])
}

/// Arguments that ask whether the task is registered.
pub fn query() -> Vec<String> {
    strings(&["/Query", "/TN", NAME])
}

fn strings(arguments: &[&str]) -> Vec<String> {
    arguments.iter().map(|a| (*a).to_string()).collect()
}

/// Whether the task is registered.
pub fn registered() -> bool {
    run(query()).is_ok()
}

/// Registers the task to start `executable` at this user's logon.
pub fn register(executable: &Path) -> Result<(), String> {
    let user = whoami()?;
    let xml = write_definition(&definition(executable, &user))?;

    let outcome = run(create(&xml));
    // The file is the scheduler's copy to read, not ours to keep; it holds
    // nothing secret, but leaving it behind would be litter Uninstall does not
    // know about.
    let _ = std::fs::remove_file(&xml);
    outcome
}

/// Removes the task. Succeeds if there was nothing to remove.
pub fn unregister() -> Result<(), String> {
    if !registered() {
        return Ok(());
    }
    run(delete())
}

/// `schtasks` reads an XML definition only as UTF-16, and says so with an error
/// that blames the XML rather than its encoding.
fn write_definition(definition: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join("footman-task.xml");

    let mut bytes = vec![0xFF, 0xFE];
    for unit in definition.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    std::fs::write(&path, bytes)
        .map_err(|error| format!("could not write the task definition: {error}"))?;
    Ok(path)
}

/// The account the task belongs to, as the scheduler names accounts.
fn whoami() -> Result<String, String> {
    let domain = std::env::var("USERDOMAIN");
    let user = std::env::var("USERNAME")
        .map_err(|_| "Windows did not say who is logged in".to_string())?;

    Ok(match domain {
        Ok(domain) if !domain.is_empty() => format!("{domain}\\{user}"),
        _ => user,
    })
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let output = Command::new("schtasks")
        .args(&arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("could not run schtasks: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    // schtasks explains itself on stderr, and its explanations are the useful
    // half of any failure here — a task name that does not exist, a definition
    // it would not read.
    let complaint = String::from_utf8_lossy(&output.stderr);
    let complaint = complaint.trim();
    Err(if complaint.is_empty() {
        format!("schtasks refused: {}", output.status)
    } else {
        complaint.to_string()
    })
}
