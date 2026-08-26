# Platform split by `cfg` modules, not by traits

## Context

Footman targets Windows first, then Linux, then macOS. The reflex for a
cross-platform project is to define a platform abstraction trait up front and
implement it per target.

## Decision

The codebase splits into a **Core** and a **Shell**.

The Core is platform-free: the Binding table, the Hyper state machine, and Chord
resolution. It consumes abstract key events and returns a Verdict plus an
optional Action. It contains no window handles, no OS calls, no clock and no I/O,
and is therefore unit-testable without an operating system.

The Shell is selected at compile time by `#[cfg(target_os = ...)]` modules
sharing a documented set of function signatures. **No trait, no dynamic
dispatch.** No stub modules are written for platforms that are not yet
implemented.

## Considered options

- **A platform trait defined now.** Rejected: an abstraction designed against a
  single implementation is reliably the wrong abstraction, and the cost is paid
  three times over once Linux and macOS arrive. The Core is already pure, so
  traits are not needed for testability either — there is nothing to mock.
- **Windows-only code now, refactor later.** Rejected: without a named boundary,
  Windows concepts (`HWND`, AUMID, virtual desktop indices) leak into the
  decision logic and the eventual extraction becomes a rewrite.

## Consequences

- **"App Identity" must be an opaque string in the Core.** Windows means an AUMID
  or an executable path; Linux will mean a `.desktop` id or `WM_CLASS`; macOS a
  bundle identifier. The Core stores and compares the string but never parses it.
  See ADR-0003.
- The Shell signatures are expected to change when the second platform lands.
  That is the plan, not a failure — they are derived from one real
  implementation and will be corrected by the second.
