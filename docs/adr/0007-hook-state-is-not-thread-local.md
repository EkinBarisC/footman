# ADR-0007: The hook's state is shared, not thread-local

## Status

Accepted. The decision stands; the explanation that first motivated it did not
survive being tested, and is recorded below as refuted rather than removed.

## Context

A `WH_KEYBOARD_LL` callback is a plain function pointer and cannot carry state,
so the Core, the Effect channel and the watchdog had to live somewhere it could
reach. They lived in a `thread_local!`, on the stated reasoning that the callback
only ever runs on the thread that installed the hook — which made a lock
unnecessary, and a lock on the path of every keystroke is what DESIGN.md §9.1
says to avoid.

That reasoning was never verified. It was an assumption the correctness of the
whole keyboard silently rested on, and the file asserted it as fact.

Slice 7 turned up a symptom that seemed to confirm it. With the settings window
open, every Chord worked as long as another application had the keyboard focus;
the moment Footman's own window took the focus, Chords stopped, and they came
back the instant it lost the focus again. Under `FOOTMAN_TRACE`:

| While Footman's own window has the focus | |
| --- | --- |
| The hook thread's message loop | still turning, once a second |
| The hook handle | still installed; nothing uninstalled it |
| The watchdog | reports the hook healthy |
| Key events reaching the callback's state | none at all |
| The keys themselves | delivered to the window; Caps Lock still toggles |

An Alt-Tab into the window caught it happening: the `Down(Tab)` was traced and
the `Up(Tab)` was not, the gap falling exactly where the window took the focus.

The reading taken from this was that Windows runs the callback on the focused
window's thread, where the `thread_local!` was empty. **That reading was wrong**,
or at least unproven: the state was moved behind a shared lock and the symptom
did not change. The count that would have settled it was itself taken too late —
after four paths on which the callback returns early — so it could not tell
"Windows did not call us" from "it called us and we declined". The question was
abandoned before the next measurement, the behaviour being one nobody wants
(see *Consequences*).

## Decision

The hook's state lives in a process-wide `Mutex`, reachable from whichever
thread the callback runs on.

The justification is not the refuted reading. It is that **Footman cannot
demonstrate which thread the callback runs on**, and the thread-local
arrangement made correctness depend on an answer nobody had. A shared cell costs
an uncontended lock and removes the dependency.

## Considered options

- **Keep the `thread_local!`.** Rejected: not because it is known to be wrong,
  but because it is not known to be right, and what it silently costs when it is
  wrong is the entire keyboard, reported as nothing at all.
- **Populate the `thread_local!` on every thread that might be called.** Rejected:
  there is no list of them. eframe, `winit` and `tray-icon` all create threads
  Footman does not own.
- **Install the hook from the user interface thread.** Rejected: it makes the
  keyboard depend on a window existing and on an event loop belonging to a
  dependency. The keyboard comes first (DESIGN.md §7).

## Consequences

- §9.1's rule stands unchanged: the callback must return within
  `LowLevelHooksTimeout`. The critical section is a table lookup and a channel
  send, and the only threads that could contend for the lock are running this
  same callback, which Windows serialises anyway — it is blocked on each answer
  before it delivers the next key.
- A poisoned lock is recovered from rather than propagated: a panic elsewhere
  must not take the keyboard with it. Nothing that takes the lock may be called
  while it is held, so the trace sender is cloned out rather than used in place.
- **Chords do not fire while Footman's own settings window holds the keyboard
  focus.** Accepted, and recorded in DESIGN.md §10: the settings window is where
  Chords are edited, not where they are used, and nobody presses a shortcut at
  the window they are configuring it in. Should it ever matter, the next
  measurement is a count taken on the callback's first line.
