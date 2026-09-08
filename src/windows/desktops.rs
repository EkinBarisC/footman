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

use std::time::Duration;

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_LEFT, VK_LWIN, VK_RIGHT};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SPI_SETCLIENTAREAANIMATION, SPIF_SENDCHANGE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::BOOL;

use windows::core::{HSTRING, PCWSTR};

use super::synthetic::{press, release, send};

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
///
/// Crossing two desktops is two switches, and Windows animates each of them:
/// asking for desktop 3 from desktop 1 slides through desktop 2 on the way.
/// There is no way to arrive directly — that is the interface ADR-0004 rejected
/// — so instead the animation is turned off for the length of the journey and
/// put back afterwards. What the user sees is arrival rather than travel.
pub fn switch_to(index: usize) -> Result<(), String> {
    let desktops = position()?;

    let step = steps_to(desktops, index)
        .ok_or_else(|| format!("there is no desktop {index}; you have {}", desktops.count))?;
    if step == Move::Stay {
        return Ok(());
    }

    let animating = animation();
    if animating {
        set_animation(false);
    }
    walk(step);
    // The presses are asynchronous: `SendInput` returns long before Windows has
    // finished moving. Turning the animation back on too early would put it
    // back in the middle of the journey, which is the flicker this avoids.
    wait_until_at(index);
    if animating {
        set_animation(true);
    }

    Ok(())
}

/// Waits for Windows to report that it has arrived.
///
/// Bounded, and silent when it expires: the arrows have been sent either way,
/// and a walk that outlasts this has already cost the user more than saying so
/// would gain them.
fn wait_until_at(index: usize) {
    const PATIENCE: Duration = Duration::from_millis(600);

    let deadline = std::time::Instant::now() + PATIENCE;
    while std::time::Instant::now() < deadline {
        if position().is_ok_and(|here| here.current == index) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
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

    // `as_chunks` rather than `chunks_exact`: the length was just checked to be
    // a multiple, so there is no remainder to think about and the compiler is
    // told the size instead of being asked to trust it.
    let position = ids
        .as_chunks::<GUID_BYTES>()
        .0
        .iter()
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

/// Whether this machine animates at all — the "Show animations in Windows"
/// setting, which is the one that governs the desktop switch.
fn animation() -> bool {
    let mut on = BOOL(0);
    let asked = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut on).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    // If Windows will not say, assume it does not animate and leave the setting
    // alone: a stray desktop sliding past is a far smaller harm than turning a
    // machine's animations off and being unable to put them back.
    asked.is_ok() && on.as_bool()
}

/// Turns the animation setting on or off for everyone, which is why every path
/// that turns it off puts it back.
fn set_animation(on: bool) {
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_SETCLIENTAREAANIMATION,
            0,
            // A boolean system parameter is set by value, not through a pointer.
            Some(usize::from(on) as *mut std::ffi::c_void),
            SPIF_SENDCHANGE,
        );
    }
}

/// Presses `Ctrl+Win+Left` or `Ctrl+Win+Right` the required number of times.
///
/// The modifiers are held down across the whole run rather than pressed once
/// per arrow: that is what a person does, and it is one switch as far as the
/// user is concerned.
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

    send(&batch);
}
