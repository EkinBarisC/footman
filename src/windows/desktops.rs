//! Switching virtual desktops by absolute index (ADR-0004).
//!
//! Windows has no documented way to do this. The one interface that can,
//! `IVirtualDesktopManagerInternal`, is undocumented and has changed shape
//! across feature updates, breaking every wrapper that bound to it. So Footman
//! reads where it is from the registry and walks there with the documented
//! `Ctrl+Win+Left` / `Ctrl+Win+Right` shortcut.
//!
//! This is the only place Footman emits synthetic input (ADR-0001). The
//! emission is bounded and single-shot, with no held state, so it does not
//! carry the stuck-modifier risk that decision rejected.

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
    VK_CONTROL, VK_LEFT, VK_LWIN, VK_RIGHT,
};

use windows::core::{HSTRING, PCWSTR};

use super::hook::FOOTMAN_SIGNATURE;

/// Where Windows keeps the desktop list. Undocumented, and verified live on the
/// reference machine: `CurrentVirtualDesktop` updates the moment the user
/// switches, not at logoff (ADR-0004).
const REGISTRY_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops";

/// Where the user is, as Windows currently reports it.
pub fn position() -> Result<Desktops, String> {
    let ids = binary_value("VirtualDesktopIDs")
        .ok_or("Windows did not report its list of virtual desktops")?;
    let current = binary_value("CurrentVirtualDesktop")
        .ok_or("Windows did not report which virtual desktop is current")?;

    desktops_from(&ids, &current)
        .ok_or_else(|| "could not make sense of the virtual desktop registry values".to_string())
}

/// Switches to a desktop by absolute index, the numbering a config file uses.
pub fn switch_to(index: usize) -> Result<(), String> {
    let desktops = position()?;

    let step = steps_to(desktops, index)
        .ok_or_else(|| format!("there is no desktop {index}; you have {}", desktops.count))?;

    walk(step);
    Ok(())
}

fn binary_value(name: &str) -> Option<Vec<u8>> {
    let path = HSTRING::from(REGISTRY_PATH);
    let name = HSTRING::from(name);
    let mut size = 0u32;

    unsafe {
        // Asking with no buffer reports the size needed.
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_BINARY,
            None,
            None,
            Some(&mut size),
        )
        .ok()
        .ok()?;

        let mut buffer = vec![0u8; size as usize];
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_BINARY,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;

        buffer.truncate(size as usize);
        Some(buffer)
    }
}

/// Where the user is among their desktops, one-based — the numbering a config
/// file uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Desktops {
    pub current: usize,
    pub count: usize,
}

/// How to get from one desktop to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Stay,
    Left(usize),
    Right(usize),
}

/// The width of a GUID in the registry's binary values.
const GUID_BYTES: usize = 16;

/// Reads the two registry values into a position.
///
/// `ids` is `VirtualDesktopIDs`, an ordered array of GUIDs; `current` is
/// `CurrentVirtualDesktop`, one of them. Both are undocumented, so anything
/// that does not fit that shape yields `None` rather than a guess — a guess
/// here would move the user somewhere they did not ask to be.
pub fn desktops_from(ids: &[u8], current: &[u8]) -> Option<Desktops> {
    if ids.is_empty() || !ids.len().is_multiple_of(GUID_BYTES) || current.len() != GUID_BYTES {
        return None;
    }

    let position = ids
        .chunks_exact(GUID_BYTES)
        .position(|guid| guid == current)?;

    Some(Desktops {
        current: position + 1,
        count: ids.len() / GUID_BYTES,
    })
}

/// How many arrow presses reach `target`, or `None` if there is no such
/// desktop.
///
/// A missing desktop is refused rather than clamped to the nearest one. Walking
/// somewhere the user did not ask for would look like a Footman bug and would
/// never tell them their config names a desktop they do not have.
pub fn steps_to(desktops: Desktops, target: usize) -> Option<Move> {
    if target == 0 || target > desktops.count {
        return None;
    }

    Some(match target.cmp(&desktops.current) {
        std::cmp::Ordering::Equal => Move::Stay,
        std::cmp::Ordering::Less => Move::Left(desktops.current - target),
        std::cmp::Ordering::Greater => Move::Right(target - desktops.current),
    })
}

/// Presses `Ctrl+Win+Left` or `Ctrl+Win+Right` the required number of times.
///
/// The modifiers are held down across the whole run rather than pressed once
/// per arrow: that is what a person does, and Windows animates each step
/// regardless.
pub fn walk(step: Move) {
    let (arrow, times) = match step {
        Move::Stay => return,
        Move::Left(times) => (VK_LEFT, times),
        Move::Right(times) => (VK_RIGHT, times),
    };

    let mut batch = vec![press(VK_CONTROL), press(VK_LWIN)];
    for _ in 0..times {
        batch.push(press(arrow));
        batch.push(release(arrow));
    }
    batch.push(release(VK_LWIN));
    batch.push(release(VK_CONTROL));

    unsafe {
        SendInput(&batch, size_of::<INPUT>() as i32);
    }
}

fn press(key: VIRTUAL_KEY) -> INPUT {
    stroke(key, Default::default())
}

fn release(key: VIRTUAL_KEY) -> INPUT {
    stroke(key, KEYEVENTF_KEYUP)
}

fn stroke(
    key: VIRTUAL_KEY,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                // Stamped so the hook recognises Footman's own work and does
                // not feed it back into the Core.
                dwExtraInfo: FOOTMAN_SIGNATURE,
            },
        },
    }
}
