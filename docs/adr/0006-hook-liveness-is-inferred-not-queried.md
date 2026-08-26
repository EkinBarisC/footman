# ADR-0006: Hook liveness is inferred, not queried

## Status

Accepted.

## Context

Windows uninstalls a low-level keyboard hook whose callback overruns
`LowLevelHooksTimeout` (300 ms by default). It does this silently: no error, no
message, no log entry. The process keeps running and simply stops seeing keys.

For Footman this is the worst failure mode available. The hook is the whole
product, and a dead hook is indistinguishable from a working one until the user
presses Hyper and nothing happens — possibly hours later, possibly in the middle
of something else.

There is no API that answers "is my hook still installed?". `SetWindowsHookExW`
returns a handle, but the handle stays valid-looking after Windows has dropped
the hook, and `UnhookWindowsHookEx` on a dropped hook succeeds. Liveness has to
be inferred from behaviour.

## Decision

A watchdog on the hook thread compares two clocks:

- the tick of the last key event **our hook** saw, and
- the tick of the last input event **the system** recorded, from
  `GetLastInputInfo`.

If the system recorded input after the last event we saw, and we have seen
nothing for a quiet period (30 s), the hook is presumed dead: it is unhooked and
reinstalled. The comparison is driven by a `WM_TIMER` on the same message loop
that delivers the callbacks, so it costs no extra thread.

The policy lives in `HookWatch`, which takes tick counts as arguments and reads
no clock of its own. It is therefore testable without an operating system or a
stopwatch, which is why the rules above are stated as five tests rather than as
a comment.

## Consequences

**Mouse-only activity produces a false positive.** `GetLastInputInfo` counts
mouse movement as input, so a user who scrolls a page for thirty seconds without
touching the keyboard looks, to the watchdog, like a machine with a dead hook.
The response is to reinstall a hook that was fine. This is accepted: reinstalling
is cheap, invisible to the user, and idempotent. The alternative — a
keyboard-only input clock — does not exist in the Win32 API.

**Detection takes up to a quiet period plus a timer tick.** A shorter quiet
period would detect faster and misfire more; 30 s is a guess, chosen to be well
beyond any plausible pause mid-sentence. It is a constant, not a setting: a user
cannot reasonably be asked to tune it.

**A hook that dies during genuine idleness is not detected until use resumes.**
That is correct rather than merely tolerable — nothing has been missed, so there
is nothing to repair, and the first keypress after the idle period supplies the
evidence.

**The watchdog cannot fix the cause.** It is a net, not a remedy. The real
defence against eviction is the callback budget in DESIGN.md §9.1: the callback
does one table lookup and one channel send, and every Effect runs on the
Dispatcher thread. `tests/decision_budget.rs` guards that budget.
