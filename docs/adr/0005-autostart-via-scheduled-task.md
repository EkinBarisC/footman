# Autostart via a per-user Scheduled Task, from a self-installed copy

## Context

The originating complaint about the tool Footman replaces was that it did not
reliably start with the machine. The default mechanism, an `HKCU\...\Run`
registry value, has three failure modes: Windows may defer it by
`StartupDelayInMSec`, the user can disable it from Task Manager's Startup tab
without realising what they disabled, and nothing restarts it if it exits.

## Decision

Footman ships as a single portable `.exe` with no installer. When autostart is
turned on it copies itself into `%LOCALAPPDATA%\Footman` and registers a per-user
**Scheduled Task** triggered at logon, with a startup delay and
restart-on-failure configured. The settings window exposes the toggle and an
**Uninstall** action that removes the task, the config directory, and the
installed copy.

## Considered options

- **`HKCU\...\Run`.** Rejected for the three failure modes above — which are
  the specific symptom being fixed.
- **A real installer (MSI/MSIX).** Rejected for v1: it adds signing, upgrade and
  repair machinery for no benefit, since a self-installing portable executable
  reaches the same end state.

## Consequences

- Creating a task in the user's own scope does not require elevation, which keeps
  Footman consistent with running unelevated (see DESIGN.md §8).
- Because the scheduled task references an absolute path, the executable cannot
  be moved freely — hence self-installation to a fixed location rather than
  running from wherever the user dropped it.
- No code signing in v1. SmartScreen will warn on first run; this is documented
  in the README rather than paid for.
- **Installing happens when autostart is turned on, not on first run**, which is
  a change from the first draft of DESIGN.md §11. A portable executable that
  copies itself somewhere on being run once is doing something the user did not
  ask for; tying it to the feature that requires it means the copy exists
  exactly when there is a reason for it to.
- The task is registered by handing `schtasks` an XML definition rather than by
  using its flags, which cannot express restart-on-failure or an unlimited run
  time. The definition must be UTF-16 or `schtasks` rejects it with a complaint
  about the XML rather than about its encoding.
- Every setting the decision was made *for* — the logon trigger, the restart, no
  time limit, starting on battery — is asserted by name in
  `tests/windows_task.rs`. A definition that merely parses would have cost the
  extra machinery without buying what it was for.
