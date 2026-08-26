# Hyper is an internal state, never emitted as real modifiers

## Context

"Hyper key" conventionally means remapping a key (usually Caps Lock) to the
simultaneous chord Ctrl+Alt+Shift+Win, so that every application on the machine
can bind shortcuts against it. Footman needs a Hyper key, and the obvious
implementation is to synthesize those four modifiers on key-down and release
them on key-up.

## Decision

Footman never emits modifiers. The Hyper Key's key-down is suppressed by the
keyboard hook and recorded as internal state; the next key event is resolved
against Footman's own Binding table. The operating system never observes a Hyper
press at all.

## Considered options

- **Synthesize Ctrl+Alt+Shift+Win.** Rejected for four reasons, in descending
  weight:

  1. **AltGr collision.** On Windows, `Ctrl+Alt` *is* AltGr. On the Turkish Q
     layout — the primary author's layout — `@ € ₺ ~ # $ { } [ ]` all live on
     AltGr. Holding a synthetic Hyper would put the system into a permanent AltGr
     state, silently corrupting text entry. This is not a bug to be patched; it
     falls out of the design.
  2. **Stuck modifiers.** If the process dies, a key-up is swallowed by a focus
     change, or a UAC/RDP transition intervenes, the machine is left with four
     modifiers held down and no way for the user to diagnose it.
  3. **The Win key opens the Start menu** on press-and-release and needs separate
     suppression hacks.
  4. **Portability.** "Observe and suppress" maps cleanly onto `WH_KEYBOARD_LL`,
     Linux evdev, and macOS `CGEventTap`. Synthetic input is a different mess on
     each of the three.

## Consequences

- **Hyper chords cannot be delegated to other applications.** A user cannot bind
  `Hyper+K` inside VS Code, because VS Code will never see the chord. Footman is
  the sole consumer of its own key namespace. This was accepted deliberately.
- Footman produces **no synthetic input at all** except where an Action requires
  it (see ADR-0004). The keyboard hook is a pure observe-and-suppress machine.
- A crash is self-healing: Windows uninstalls the hook with the process, and the
  keyboard immediately returns to normal. There is no state that can be left
  corrupted.
- No timers are needed anywhere in the trigger path. Tap versus chord is not a
  duration measurement — it is simply "did a Chord fire before Hyper was
  released?"
