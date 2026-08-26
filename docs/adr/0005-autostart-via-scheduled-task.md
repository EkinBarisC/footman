# Autostart via a per-user Scheduled Task, from a self-installed copy

## Context

The originating complaint about the tool Footman replaces was that it did not
reliably start with the machine. The default mechanism, an `HKCU\...\Run`
registry value, has three failure modes: Windows may defer it by
`StartupDelayInMSec`, the user can disable it from Task Manager's Startup tab
without realising what they disabled, and nothing restarts it if it exits.

## Decision

Footman ships as a single portable `.exe` with no installer. On first run it
copies itself into `%LOCALAPPDATA%` and registers a per-user **Scheduled Task**
triggered at logon, with a startup delay and restart-on-failure configured. The
settings window exposes a toggle for this and an **Uninstall** action that removes
the task, the config directory, and the installed copy.

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
