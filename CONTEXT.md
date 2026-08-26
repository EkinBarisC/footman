# Footman

A cross-platform hyper-key launcher. One key becomes Hyper; Hyper plus a key
runs an Action — focus an app, open a target, run a command, switch desktop.
No search palette, no window management, no text expansion.

## Language

### The trigger

**Hyper**:
The role a single physical key is promoted to while Footman runs. Held Hyper
suspends the normal keyboard and opens Footman's own key namespace. It is never
emitted to the system as modifiers — it exists only inside Footman.
_Avoid_: leader, prefix, meta, super, modifier

**Hyper Key**:
The physical key currently assigned the Hyper role. Caps Lock by default.
_Avoid_: trigger key, activation key

**Tap**:
Pressing and releasing the Hyper Key without any Chord firing in between.
Resolves to a configured Tap Action: nothing, Escape, or a real Caps Lock toggle.
_Avoid_: short press, single press, click

**Chord**:
Hyper, plus zero or more ordinary modifiers, plus exactly one key. Footman's unit
of input. Chords are flat — there are no multi-key sequences or layers.
_Avoid_: shortcut, hotkey, combo, keybind, sequence

**Verdict**:
Footman's ruling on one key event: **Pass** (the system sees it untouched) or
**Suppress** (it is dropped and never reaches the foreground app).
_Avoid_: decision, result, action

### The response

**Binding**:
One Chord mapped to one Action. The atom of a user's configuration.
_Avoid_: mapping, rule, entry, command

**Action**:
What a Binding does when its Chord fires. Exactly four kinds exist:
`app`, `open`, `run`, `desktop`.
_Avoid_: task, handler, effect, job

**App Action**:
Focus an application, launching it first if it is not running. Repeating the
Chord while that application is already frontmost moves to its next window.
_Avoid_: launch, focus, activate, switch — the point is that it is all three

**Open Action**:
Hand a URL, file, or folder to the operating system's default handler.
_Avoid_: browse, navigate, start

**Run Action**:
Execute a command line, with the console window shown or hidden.
_Avoid_: exec, shell, script

**Desktop Action**:
Switch to a virtual desktop by absolute index.
_Avoid_: workspace, space, screen

### Identity

**App Identity**:
The opaque string that names an application to the platform. The Core never
interprets it. On Windows it is an AUMID; on Linux a `.desktop` id or `WM_CLASS`;
on macOS a bundle identifier.
_Avoid_: app name, executable, process name, path

### Structure

**Core**:
The platform-free half of Footman. Holds the Binding table and the Hyper state
machine; consumes abstract key events and produces a Verdict plus an optional
Action to dispatch. Contains no window handles and no operating system calls.
_Avoid_: engine, brain, logic, domain

**Shell**:
The platform-specific half. Watches the keyboard, enumerates and launches
applications, finds and raises windows, changes desktops. Selected at compile
time; never abstracted behind a trait while only one exists.
_Avoid_: backend, adapter, driver, platform layer

**Dispatcher**:
The worker that executes Actions off the hook thread. Exists because the hook
thread must return within the operating system's timeout or the hook is silently
uninstalled.
_Avoid_: executor, runner, queue
