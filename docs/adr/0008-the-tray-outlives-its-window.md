# ADR-0008: The tray is rebuilt when it dies; the process is not

## Status

Accepted.

## Context

Footman sometimes vanished after the laptop woke: no tray icon, no Chords, and
nothing to say why. The machine it happened on uses Modern Standby, which falls
through to hibernation when the standby battery budget runs out ("Hibernate from
Sleep — Standby Battery Budget Exceeded" in the System log), and has hybrid
graphics — an NVIDIA GPU beside an AMD one.

The process was not crashing in the sense Windows records. There was no
Application Error event and no Windows Error Reporting entry for the installed
build. What there was is this, in eframe's OpenGL integration:

- On a resize — which resuming produces, hidden window or not — eframe makes its
  GL context current before resizing the surface, and does so with `unwrap()`
  (`glow_integration.rs`, `change_gl_context`).
- A context that did not survive hibernation cannot be made current. The
  `unwrap` panics inside `winit`'s window procedure; `winit` catches it, resets
  its runner, and re-raises it on the main thread once the loop returns.
- The panic unwinds out of `run_native` and out of `main`. The hook thread dies
  with the process, the tray icon with it. Exit code 101, which to Windows is an
  ordinary exit. The message went to standard error, which a Windows-subsystem
  process started by the scheduler does not have.

The hook never needed any of this. It runs on its own thread with its own
message loop (ADR-0007) and has no GL context to lose.

## Decision

The user interface is run under `catch_unwind`, and when it dies — by panic or
by `run_native` returning an error — it is built again, in the same process,
while the hook thread carries on untouched.

This works because eframe keeps its `winit` event loop in a thread-local and
reuses it when `run_and_return` is set, and `winit` resets its runner before
re-raising a panic. The rebuilt window gets a fresh GL context, which is the
thing that was lost.

A rebuilt window re-reads the config from disk, so it shows what was last saved.
Unsaved edits in a window that died are lost; so would they be to a crash.

Rebuilding is bounded by `Revival`: the first waits two seconds, each one after
that inside ten minutes waits twice as long up to a minute, and a ninth death
inside ten minutes gives up. Giving up takes the hook down and exits — a hook
nobody can pause or quit is not one to leave running.

The counts are measured rather than guessed. On the machine this was written
for, the first rebuilds after a wake fail with "found no glutin configs matching
the template": there is no GPU to give the window one yet, and the graphics
stack took some thirty seconds to come back. Five attempts capped at thirty
seconds nearly ran out on the real thing, so it is eight capped at a minute.

Every death, every rebuild, and every panic on any thread is written to
`footman.log` beside the config file.

## Considered options

- **Relaunch the executable.** Rejected: it drops the hook while the new process
  starts, and needs its own guard against a relaunch loop. Rebuilding in place
  costs the keyboard nothing.
- **Switch eframe to `wgpu`.** Rejected as the fix: it moves the problem rather
  than removing it — a lost device is still lost — and it is a much larger
  dependency. It would not change this decision.
- **Rely on the Scheduled Task's restart-on-failure.** Rejected: it covers a task
  that fails to start, not a process that exits after running for a day. Nor is
  it there for a copy started by hand.

## Consequences

- The window's death is a non-event for the keyboard. Chords work throughout,
  including in the seconds before the tray icon reappears.
- The heartbeat thread that keeps a hidden window's event loop turning stops
  with the window it belongs to, so rebuilds do not accumulate them.
- A panic is now recorded rather than lost. The next failure of this kind that
  is not this one will say what it was.
- **The window has to insist on being hidden.** eframe builds a window hidden
  and shows it as soon as it has painted a frame, and it paints one because the
  first frame's `ViewportInfo` does not know the window is hidden yet — its
  `visible()` is `None` there, which is read as visible. The settings window
  therefore appeared at every logon, and again after every rebuild. So `logic`
  re-sends `Visible(false)` on every frame until the user actually asks for the
  window. Measured both ways: hidden across a rebuild, and still opening when
  the tray asks for it.
