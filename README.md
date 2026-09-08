# Footman

[![CI](https://github.com/EkinBarisC/footman/actions/workflows/ci.yml/badge.svg)](https://github.com/EkinBarisC/footman/actions/workflows/ci.yml)

A hyper-key launcher. One key becomes **Hyper**; Hyper plus a key focuses an
application, opens a target, runs a command, or switches virtual desktop.

Hold Caps Lock, press `C`, and you are in Chrome — launched if it was not
running, and stepped to its next window if you were already there. That is the
whole product.

Footman replaces one specific slice of a launcher suite: the keyboard
shortcuts. There is no search palette, no window management, no text expansion,
no clipboard history, no plugin runtime. The workflow it is built for is muscle
memory, not search.

**Status: Windows, one release away from v0.1.** Linux is second and macOS
third; the platform-free half of the code is written to carry over unchanged.

## Getting it

Download `footman.exe` from the [releases][releases] and run it. There is no
installer and nothing to set up — the icon appears in the tray and Footman is
watching the keyboard.

The executable is not code-signed, so SmartScreen will warn the first time:
choose **More info**, then **Run anyway**. Signing costs money and proves
nothing about the code, which you can read here instead.

Footman does not copy itself anywhere until you ask it to. Turning on **Start
with Windows** in the settings window (or running `footman install`) installs a
copy under `%LOCALAPPDATA%\Footman` and registers a per-user Scheduled Task to
start it at logon — a logon trigger has to name an absolute path, and the folder
you happened to download into is not one to build one on. **Uninstall Footman…**
in the same window, or `footman uninstall`, takes all of it away again: the
task, the config file and the installed copy, leaving nothing.

[releases]: https://github.com/EkinBarisC/footman/releases

## Using it

The tray icon has three entries: **Settings…**, **Pause Footman** and **Quit**.

Pause uninstalls the keyboard hook rather than ignoring events, so the keyboard
is entirely normal while it is paused — for gaming, screen sharing, or handing
the keyboard to someone else. It has no shortcut of its own on purpose: a
shortcut that disables shortcuts is a trap.

The settings window has two tabs.

**Bindings** is a table of Chord, Action and target. A Chord is recorded by
clicking the cell and *pressing the key* — you never type a key name as a
string, and modifiers held while pressing are recorded with it. Choosing an
`app` Action opens the list of installed applications with a filter box, and
resolves the identity behind the scenes.

**General** holds the Hyper Key, the Tap Action, **Start with Windows**, where
the config file is, and Uninstall.

Saving takes effect at once. The keyboard hook is not reinstalled for it, so
pressing Save never makes the keyboard flicker.

## Chords

A Chord is Hyper, plus zero or more ordinary modifiers, plus exactly one key:

```
Hyper+C          Hyper+Shift+C          Hyper+1
```

Chords are flat. There are no sequences, no layers and no timers anywhere in the
trigger path — Footman has no hold threshold and no chord timeout, and therefore
no state it can get stuck in.

**While Hyper is held, the normal keyboard is suspended.** A key with no Binding
is suppressed, not passed through. Passing unbound keys through would mean an
accidental Hyper press quietly typing into whatever has focus, which is hard to
notice and harder to attribute; suppression's worst case is that nothing
happens, which is diagnosable.

**One Action per physical press.** Windows delivers roughly thirty key-downs a
second while a key is held. An Action fires once and does not re-fire until the
key is released.

**Tap** is releasing the Hyper Key without any Chord having fired in between. It
is not a duration — nothing is timed — it is only "did a Chord fire before Hyper
came back up?". A Tap resolves to the configured Tap Action: `none`, or
`escape`.

Note what the default costs you: with Hyper on Caps Lock, Caps Lock no longer
toggles capitals. `tap = "escape"` puts Escape under your left little finger,
which is most of why people move Caps Lock in the first place.

## Actions

| Kind | Does | Example |
| --- | --- | --- |
| `app` | Focus an application, launching it if it is not running | Chrome |
| `open` | Hand a URL, file or folder to the OS default handler | `https://github.com` |
| `run` | Execute a command line, console shown or hidden | `pnpm dev` |
| `desktop` | Switch to a virtual desktop by absolute index | `1` |

The App Action is the one with real behaviour behind it. Not running, it
launches. Running with a window on the desktop you are looking at, it raises the
one nearest the top of the Z-order. Running only elsewhere, it switches to that
desktop and raises the window there — so a user who partitions work across
desktops usually never leaves the one they are on. Already frontmost, it steps
to the next window of the same application *on this desktop*, wrapping around;
the cycle stays on one desktop deliberately, because one that spanned them would
fling you between contexts on repeated presses.

Modifiers do not compose Actions. `Hyper+Shift+C` is an independent Binding, not
a variation of `Hyper+C`.

## Configuration

One TOML file, at `%APPDATA%\footman\config.toml`.

```toml
[hyper]
key = "CapsLock"        # any key name below
tap = "none"            # none | escape

[general]
autostart = false

[[binding]]
chord  = "C"
action = { type = "app", id = "aumid:Chrome" }

[[binding]]
chord  = "Shift+C"
action = { type = "app", id = 'path:C:\Program Files\Unity Hub\Unity Hub.exe' }

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

**The file is the single source of truth.** The settings window writes it and
holds nothing back, so hand-editing, version control and copying a config
between machines all work for free — there is no second copy of the state to
keep in sync. Edits made in the settings window take effect as they are saved;
edits made to the file by hand are picked up the next time Footman starts, since
it does not yet watch the file.

A `chord` is a key name, optionally preceded by `Shift+`, `Ctrl+` and `Alt+` in
any order. Hyper is not written: every Binding begins with it. Win is
deliberately not available as a modifier — unsuppressed it opens the Start menu,
and suppressing it would mean synthesising input.

Key names are `A`–`Z`, `0`–`9`, `F1`–`F24`, `Left`, `Right`, `Up`, `Down`,
`Space`, `Enter`, `Tab`, `Backspace`, `Escape`, `Delete`, `Insert`, `Home`,
`End`, `PageUp`, `PageDown`, `CapsLock`.

An `app` Action's `id` is an App Identity: an opaque, scheme-prefixed string
that names an application to Windows — `aumid:Chrome`, or
`path:C:\...\Unity Hub.exe`. You are not meant to type these; the settings
window picks them, and `footman windows` prints the ones every open window
answers to.

### When something is wrong

The governing rule is that **the keyboard comes first**. Wherever Footman cannot
be certain, Hyper is not installed at all and the keyboard behaves entirely
normally.

| Situation | Behaviour |
| --- | --- |
| No config file (first run) | Write a default config with no Bindings, and sit in the tray |
| Malformed TOML | Load nothing, install no hook; the settings window reports the problem with a line number |
| One invalid Binding | Skip it, keep the rest, flag it in the settings window |
| Duplicate Chord | First wins; the second is reported as a warning, never silently applied |
| Hook cannot be installed | The tray enters an error state and retries. Never silently dead |

Malformed TOML is deliberately fatal rather than best-effort: "half my Bindings
work and I don't know why" is undiagnosable, while "nothing works and the tray
says why" is a five-second fix.

## Command line

`install`, `uninstall` and `where` are features — what is installed is a copy of
the executable you are holding, and a terminal is where you are holding it. The
rest are diagnostics: they run one piece of Footman without the keyboard.

| Command | Does |
| --- | --- |
| `footman` | Loads the config, starts the hook, sits in the tray |
| `footman install` | Installs a copy and registers the logon task |
| `footman uninstall` | Removes the task, the config directory and the copy |
| `footman where` | Says where all three are, and whether they exist |
| `footman windows` | Every window Footman can see, with the identities it answers to |
| `footman apps` | Every installed application, with the identity that names it |
| `footman focus <identity>` | Runs one App Action, without pressing a Chord |
| `footman desktop [index]` | Says which virtual desktop you are on, and switches |

Footman is built for the Windows subsystem so that the logon task does not put a
terminal on screen, and attaches to the console it was typed into when there is
one. Your shell does not wait for a process like that, so the prompt can come
back before the output does.

## Limitations

**Footman runs unelevated**, so its keyboard hook cannot see input while an
elevated window has focus: Hyper is inert over Task Manager, an administrator
terminal, an installer and UAC prompts. Running elevated instead would put a
keyboard hook at administrator privilege and hand that privilege to everything
Footman launches, which is worse. The correct answer is a `uiAccess` manifest,
which needs an Authenticode signature and installation under `C:\Program Files`
— disproportionate for now, and nothing in the architecture forecloses it.

**Chords do not fire while the settings window itself holds keyboard focus.**
This is the window where Chords are edited, not the one where they are used, and
every other window — including one sitting behind it — is unaffected. The cause
was chased and not settled; it is written up in [ADR-0007][adr7].

**Virtual desktop switching reads Windows' own registry state and sends the
arrow keys** it would send you, because the documented API refuses to move
another process's window between desktops. The registry layout has been verified
on one machine and is not guaranteed across Windows editions ([ADR-0004][adr4]).

[adr4]: docs/adr/0004-virtual-desktop-via-registry-and-synthetic-arrows.md
[adr7]: docs/adr/0007-hook-state-is-not-thread-local.md

## Building

Rust 1.96 or newer, on Windows.

```
cargo build --release
cargo test
```

The tests need no keyboard and no operating system to speak of: the Core is a
library precisely so that its invariants can be stated as properties and checked
without one. The few tests that do touch Windows read its state rather than
driving it.

## Reading

- [DESIGN.md](DESIGN.md) — what Footman is, what it deliberately is not, and why
  each part works the way it does
- [CONTEXT.md](CONTEXT.md) — the vocabulary, used exactly, in the documents and
  in the code
- [docs/adr](docs/adr) — the decisions with lasting consequences, and the
  measurements behind them

## License

MIT. See [LICENSE](LICENSE).
