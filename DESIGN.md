# Footman — Design

A cross-platform hyper-key launcher. One key becomes **Hyper**; Hyper plus a key
runs an **Action** — focus an app, open a target, run a command, switch desktop.

Vocabulary is defined in [CONTEXT.md](./CONTEXT.md) and used exactly, here and in
code. Decisions with lasting consequences are recorded in [docs/adr](./docs/adr).

---

## 1. Scope

Footman replaces one specific slice of a launcher suite: the keyboard shortcuts.
Everything that made those suites heavy is deliberately absent.

**In scope**

- A Hyper Key, assigned to a physical key, active system-wide
- Flat Chords mapping to four kinds of Action
- A settings window for editing Bindings, reachable from a tray icon
- Reliable autostart, and a Pause switch

**Out of scope — permanently**

- A search palette. This is the single largest source of weight in comparable
  tools, and the workflow Footman is built for is muscle memory, not search.
- Window management (tiling, snapping, moving between monitors)
- Text expansion, clipboard history, calculators, plugin runtimes
- Key remapping in the general sense. Footman binds Chords to Actions; it is not
  a keyboard layout tool.

**Deferred, not rejected**

- Layered Chord sequences (`Hyper a c`). The flat namespace holds 36 Bindings
  without modifiers and 72 with Shift, which exceeds the intended use. Layers
  also tend to require an on-screen hint overlay, which would put a UI back on
  the hot path.
- Volume and media Actions
- uiAccess, which would let Footman work over elevated windows (see §8)

## 2. The trigger

### 2.1 Hyper

Hyper is internal state, never emitted to the system as modifiers — see
[ADR-0001](./docs/adr/0001-hyper-is-internal-never-emitted.md). The Hyper Key's
key-down is suppressed and recorded; the system never observes a Hyper press.

Exactly one Hyper Key exists. It defaults to Caps Lock and is configurable.

### 2.2 Chords

A Chord is Hyper, plus zero or more ordinary modifiers, plus exactly one key.
Chords are flat; there are no sequences.

```
Hyper+C          Hyper+Shift+C          Hyper+1
```

### 2.3 Rules

**While Hyper is held, the normal keyboard is suspended.** A key with no Binding
is **suppressed**, not passed through. Passing unbound keys through would mean an
accidental Hyper press quietly types characters into whatever has focus — a
silent corruption that is hard to notice and harder to attribute. Suppression's
worst case is that nothing happens, which is diagnosable.

**One Action per physical press.** Windows delivers roughly thirty key-down
events per second while a key is held. An Action fires once per press and does
not re-fire until the key is released. There is no per-Action exception to this.

**Tap.** Releasing the Hyper Key without any Chord having fired resolves to the
configured Tap Action: `none` (default), `escape`, or a real `capslock` toggle.

**No timers anywhere in the trigger path.** Tap is not a duration measurement —
it is "did a Chord fire before Hyper was released?". There is no hold threshold,
no chord timeout, and therefore no state Footman can get stuck in.

## 3. Actions

| Kind | Does | Example |
| --- | --- | --- |
| `app` | Focus an application, launching it if needed | Chrome |
| `open` | Hand a URL, file or folder to the OS default handler | `https://github.com` |
| `run` | Execute a command line, console shown or hidden | `pnpm dev` |
| `desktop` | Switch to a virtual desktop by absolute index | `1` |

Modifiers do not compose Actions. Each Chord maps to exactly one Action;
`Hyper+Shift+C` is an independent Binding, not a variation of `Hyper+C`.

`desktop` is implemented through registry state plus synthetic arrow presses —
see [ADR-0004](./docs/adr/0004-virtual-desktop-via-registry-and-synthetic-arrows.md).
It is the only Action that emits synthetic input.

## 4. App Action semantics

The App Action is the heart of the product and the only one with real edge cases.

**Not running** → launch it.

**Running, one or more windows on the current desktop** → raise the one nearest
the top of the Z-order. This matches Alt-Tab intuition and needs no tracked
state.

**Running, but only on another desktop** → switch to that desktop and raise the
window there. Searching the current desktop first means that a user who
partitions work across desktops usually never leaves the one they are on; when
they do leave, it is because the application genuinely is not here.

**Already frontmost** → move to the next window *of the same application on the
current desktop*, wrapping around. With a single window on this desktop,
repeating the Chord does nothing.

The cycle is deliberately confined to the current desktop. A cycle that spanned
desktops would fling the user between contexts on repeated presses.

**Which desktop a launch lands on is Windows' decision, not Footman's.** A new
window opens on the desktop that is active when it is created, which is the
desired behaviour; single-instance applications that restore a remembered window
instead are behaving as they do for any other launcher. Footman cannot correct
this after the fact: `IVirtualDesktopManager::MoveWindowToDesktop` was measured
returning `E_ACCESSDENIED` for every window this process does not own, so moving
someone else's window between desktops is not available at all. `GetWindowDesktopId`
and `IsWindowOnCurrentVirtualDesktop`, which only read, do work across processes.

## 5. App Identity

An App Identity is an opaque, scheme-prefixed string —
`aumid:Chrome`, `path:C:\Program Files\Unity Hub\Unity Hub.exe`. The Core stores
and compares it; only the Shell parses it.

On Windows it is resolved by a three-step cascade — window AUMID, then process
AUMID, then executable path. Both obvious single-mechanism approaches were
measured and found broken; the cascade was measured at full coverage. The
evidence and reasoning are in
[ADR-0003](./docs/adr/0003-app-identity-resolution-cascade.md).

The cascade decides which identity is *stored*. For *matching*, a window answers
to every identity its cascade produces, not only the first: a Binding written as
`path:...\chrome.exe` still matches a Chrome window whose window-AUMID resolves
first. This costs nothing and removes a whole class of "it stopped matching and
I cannot see why".

One further rule, measured rather than reasoned: Windows keeps many top-level
windows alive that no user would call a window — suspended UWP applications, the
touch-keyboard host, the inner half of a UWP split window. They pass every
classic Alt-Tab test and are separated from real windows only by DWM cloaking,
and cloaking alone is not enough, because every window on another virtual
desktop is cloaked too. A window is a ghost when it is cloaked *and* on the
desktop the user is looking at. The measurements are in
`tests/windows_ghosts.rs`.

Users never type an identity. The settings window lists installed applications
and resolves the identity behind the scenes. Until it exists, `footman windows`
prints the same enumeration and the same cascade the App Action uses, and
`footman focus <identity>` runs one App Action without the keyboard.

## 6. Configuration

A single TOML file at the platform config directory —
`%APPDATA%\footman\config.toml` on Windows.

```toml
[hyper]
key = "CapsLock"
tap = "none"            # none | escape | capslock

[general]
autostart = true

[[binding]]
chord  = "C"
action = { type = "app", id = "aumid:Chrome" }

[[binding]]
chord  = "1"
action = { type = "desktop", index = 1 }

[[binding]]
chord  = "G"
action = { type = "open", target = "https://github.com" }

[[binding]]
chord  = "P"
action = { type = "run", command = "pnpm dev", show_window = false }
```

**The file is the single source of truth.** The settings window writes it;
Footman watches it and reloads live. Hand-editing, version control and copying a
config between machines therefore all work for free, and there is no second copy
of the state to keep in sync.

**There is no Save button.** Changes apply as they are made. A Save button would
create a window in which the file has been edited externally while the UI holds
unsaved changes — a conflict worth designing away rather than resolving.

## 7. Failure behaviour

The governing rule, and the analogue of Bouncer's fail-open:

> **The keyboard comes first.** Footman never leaves the user with a broken
> keyboard. Wherever Footman cannot be certain, Hyper is not installed at all and
> the keyboard behaves entirely normally.

| Situation | Behaviour |
| --- | --- |
| No config file (first run) | Write a default config with no Bindings, open the settings window |
| Malformed TOML | Load nothing, install no hook. Tray shows an error state; the settings window reports the problem with a line number |
| One invalid Binding | Skip it, keep the rest, flag it in the settings window |
| Duplicate Chord | First wins; the second is reported as a warning, never silently applied |
| Hook cannot be installed | Tray enters an error state, retries with backoff, notifies the user. Never silently dead |
| App Identity no longer resolves | The Chord is still suppressed; the Action fails with a notification and the Binding is flagged as broken |

Malformed TOML is deliberately fatal rather than best-effort. "Half my Bindings
work and I don't know why" is an undiagnosable state; "nothing works and the tray
tells me why" is a five-second fix.

Because Hyper is never emitted (ADR-0001), a crash is self-healing: Windows
uninstalls the hook with the process and the keyboard returns to normal.

## 8. Process model and privileges

**Footman runs unelevated.** The consequence is that a low-level keyboard hook
cannot observe input while an elevated window has focus, so Hyper is inert over
Task Manager, an administrator terminal, an installer, or a UAC prompt.

The alternatives were rejected for v1. Running elevated puts a keyboard hook at
administrator privilege and — worse — causes applications launched by Footman to
inherit elevation. A `uiAccess` manifest is the correct answer but requires an
Authenticode signature and installation under `C:\Program Files`, which is
disproportionate at this stage. Nothing about the architecture forecloses it.

**Autostart is a per-user Scheduled Task**, not a `Run` registry value, from a
copy of the executable that Footman installs into `%LOCALAPPDATA%` on first run.
See [ADR-0005](./docs/adr/0005-autostart-via-scheduled-task.md).

## 9. Architecture

Three parts, following the split in
[ADR-0002](./docs/adr/0002-core-shell-split-without-traits.md).

**Core** — platform-free. The Binding table, the Hyper state machine, Chord
resolution. Input: abstract key events. Output: a Verdict (`Pass` / `Suppress`)
plus an optional Action to dispatch. No OS calls, no clock, no I/O. Fully
unit-testable without an operating system.

**Shell** — platform-specific, selected by `cfg`. Watches the keyboard,
enumerates and launches applications, finds and raises windows, changes desktops.

**Dispatcher** — a worker thread that executes Actions.

### 9.1 The hook thread constraint

This is not negotiable and shapes the whole runtime:

> Windows silently uninstalls a low-level keyboard hook whose callback exceeds
> `LowLevelHooksTimeout` (300 ms by default). There is no error, no log and no
> notification. The application keeps running and simply stops seeing keys.

Launching an application, enumerating windows, or switching a desktop on the hook
thread will trigger this. Therefore:

- The hook thread only **decides** — a table lookup, microseconds — and posts the
  resulting Action to the Dispatcher over a channel.
- Every Action executes on the Dispatcher thread.
- No lock is ever acquired while deciding.
- A watchdog periodically verifies the hook is alive and reinstalls it if not.

## 10. Settings window and tray

One window, two tabs, opened from the tray. Built with egui, as in Bouncer.

**Bindings** — a table of Chord, Action, target, with edit and delete. A Chord is
recorded by clicking the cell and **pressing the key**; Hyper does not need to be
held, since every Binding begins with it. Modifiers held during capture are
recorded. `Esc` cancels. This costs nothing because the hook already exists — the
user never types a key name as a string.

**General** — Hyper Key, Tap Action, autostart toggle, config file path,
Uninstall.

Choosing an `app` Action opens a list of installed applications with a filter
box. This is not a palette: it lives inside the settings window, is used only
while creating a Binding, and never appears on the hot path. An escape hatch
allows picking an `.exe` or `.lnk` directly.

**Tray menu**: `Settings…`, `Pause Footman`, `Quit`. Pause uninstalls the hook
entirely, returning the keyboard to normal — for gaming, screen sharing, or
handing the keyboard to someone else. The tray icon reflects state, as in
Bouncer. **Pause has no keyboard shortcut**: a shortcut that disables shortcuts
is a trap, and pressing it accidentally leaves the user unable to work out why
nothing responds.

## 11. Distribution

Open source, MIT, GitHub, CI — the Bouncer skeleton.

A single portable `.exe`, no installer; it self-installs to `%LOCALAPPDATA%` on
first run (§8). **Uninstall** in the settings window removes the scheduled task,
the config directory and the installed copy, leaving no trace.

No code signing in v1; SmartScreen will warn and the README will say so. Package
manager manifests come after the product settles. The interface is English only;
no localisation.

## 12. Build order

Each slice states how it is verified. Slices 0-2 are complete.

| # | Slice | Verification |
| --- | --- | --- |
| 0 | **Identity spike** — done | Measured: window AUMID 2/7, process AUMID covers the rest, path covers the remainder. Result recorded in ADR-0003 |
| 1 | **Core** — done — Hyper state machine, Binding table, Chord resolution | Unit and property tests: auto-repeat fires exactly once, unbound keys always Suppress, Tap only when no Chord fired |
| 2 | **Config** — done — schema, load, validate, report | Round-trip tests; malformed fixtures match the §7 table |
| 3 | **Hook + Dispatcher** — done — hook, channel, worker, watchdog | Callback budget measured; liveness inferred rather than queried (ADR-0006); end-to-end verified by hand, since `SendInput` does not reach hooks on the development machine |
| 4 | **App Action** — done — find, raise, cycle, cross-desktop rule | Manual matrix: running/not × one window/several × this desktop/another; cascade and ghost rule measured on the reference machine |
| 5 | **Open, Run, Desktop Actions** — done | Manual; desktop index verified against the registry |
| 6 | **Tray + Pause** — done — and the Tap Action, which no slice had claimed | Manual: the icon changes state, Pause returns the keyboard to normal, Resume restores it |
| 7 | **Settings window** | Manual; Chord capture and application picker |
| 8 | **Self-install, Scheduled Task, Uninstall** | Install and uninstall on a clean VM, verify no residue |
| 9 | **README, CI, release** | — |

The Core comes first so there is a testable heart before any OS is involved. The
settings window comes last so it is drawn against a settled model rather than
redrawn against a moving one.

## 13. Beyond Windows

Linux is second, macOS third. The Core is expected to carry over unchanged; the
Shell is written per platform.

Known shape of the work: Linux needs evdev/uinput for input and splits between
X11 (EWMH is well specified) and Wayland (compositor-specific, and the hard
case); App Identity becomes `desktop:` or `class:`. macOS needs `CGEventTap` and
Accessibility permission, with App Identity as `bundle:`; Spaces switching is
restricted and may not be expressible.

Shell function signatures will be corrected when the second platform lands. That
is the plan, not a regression.

## 14. Open questions

- Whether a window's AUMID and its process's AUMID can disagree in practice, and
  whether preferring the window's is always right (§5)
- Whether Chrome reports a distinct window AUMID per profile, and whether that is
  desirable or surprising
- Whether the virtual-desktop registry layout is stable across Windows editions
  other than the reference machine (ADR-0004)
