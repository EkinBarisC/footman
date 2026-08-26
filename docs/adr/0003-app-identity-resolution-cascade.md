# App Identity is resolved by a three-step cascade, stored scheme-prefixed

## Context

The App Action must answer "is this application already running, and which of its
windows is it?" That requires a stable name for an application — one that
survives updates and works for both classic Win32 and MSIX/UWP applications.

Both obvious candidates were measured on a live machine and **both were broken**:

- **Executable path.** MSIX packages carry their version *inside the install
  path* — `...\WindowsApps\Raycast.Raycast_2.0.5.0_x64__qypenmj9wpt2a\...`. Every
  application update would silently break the Binding.
- **Process name.** UWP windows are owned by `ApplicationFrameHost.exe`, which
  says nothing about the application. Generic hosts are worse: Raycast's backend
  runs as `node.exe`.

A third candidate, the window's AUMID via `SHGetPropertyStoreForWindow`, resolved
only **2 of 7** real windows in measurement — Chrome and a UWP app, but not
Claude, DuckDuckGo or Unity Hub.

## Decision

Resolve App Identity by cascade, first hit wins:

1. **Window AUMID** — `SHGetPropertyStoreForWindow` + `PKEY_AppUserModel_ID`
2. **Process AUMID** — `GetApplicationUserModelId` on the owning process
3. **Executable path** of the owning process

Store the result as a single opaque, scheme-prefixed string:
`aumid:Chrome`, `path:C:\Program Files\Unity Hub\Unity Hub.exe`. The Core treats
it as an opaque key; only the Shell parses the scheme.

## Consequences

- **The two AUMID sources cover each other's blind spots exactly.** Step 1
  catches UWP applications hidden behind `ApplicationFrameHost` and applications
  that set a window AUMID for taskbar pinning (Chrome). Step 2 catches every
  MSIX-packaged application — which is precisely the set whose executable path
  contains a version number. Step 3 catches unpackaged Win32 applications, whose
  paths are stable. Measured coverage on the reference machine: 7 of 7.
- Window AUMID is tried before process AUMID deliberately: it is what the Windows
  taskbar uses to group windows, so it matches what a user perceives as "the same
  app". It also lets a Chrome PWA be bound separately from Chrome itself.
- New platforms add new schemes (`desktop:`, `class:`, `bundle:`) without any
  change to the Core or to the config schema.
- Users never type an identity. The settings window lists installed applications
  and runs the cascade behind the scenes.
