# Absolute virtual desktop switching via registry state plus synthetic arrows

## Context

The Desktop Action switches to a virtual desktop by **absolute** index
("Hyper+1 → desktop 1"). Windows exposes no documented API for this. The
documented `IVirtualDesktopManager` can query and move windows but cannot switch
desktops. The only interface that can, `IVirtualDesktopManagerInternal`, is
undocumented and its GUID and vtable layout have changed across Windows feature
updates, breaking every open-source wrapper repeatedly.

## Decision

Read the current desktop index from the registry, then send that many synthetic
`Ctrl+Win+Left` / `Ctrl+Win+Right` presses.

State comes from `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops`:
`VirtualDesktopIDs` is an ordered array of 16-byte GUIDs, and `CurrentVirtualDesktop`
holds the active one. The index is the position of the latter in the former.

## Considered options

- **Bind to `IVirtualDesktopManagerInternal`.** Rejected: it would impose a
  permanent per-Windows-build maintenance tax and reintroduce exactly the
  "stopped working after an update" failure mode Footman exists to avoid.
- **Relative switching only** (`next` / `previous` desktop). Rejected: it does not
  express the binding the user actually wants. Tracking position by counting
  Footman's own switches desynchronises permanently the moment the user switches
  by touchpad gesture or by pressing the shortcut directly.

## Consequences

- This is the one place Footman produces synthetic input (cf. ADR-0001). It is a
  bounded, single-shot emission of a documented shortcut, with no held state, so
  it does not carry the stuck-modifier risk that decision rejected.
- Switching costs `|target - current|` keystrokes and animations. Acceptable at
  the 3-6 desktops people actually use; it would be poor at 20.
- **Crossing two desktops is two switches, because it is.** Desktop 1 to desktop
  3 travels through desktop 2, and there is no way to arrive directly without
  the interface this decision rejected. What can be removed is the *drawing* of
  the journey: `switch_to` turns "Show animations in Windows" off for the length
  of the walk and puts it back, so the user sees arrival rather than travel.
  Pacing the arrows instead was tried first and rejected — it made the stutter
  slower without making it fewer desktops.
- Turning that setting off is machine-wide for the tenth of a second it lasts,
  and is restored on every path. It is the same bargain `apps::raise` already
  strikes with the foreground lock timeout, and the same obligation.
- The registry values were verified as **live** on the reference machine —
  `CurrentVirtualDesktop` updated immediately on a manual desktop switch, not at
  logoff. If a future Windows release breaks this, the fallback is relative-only
  switching.
