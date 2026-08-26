//! The low-level keyboard hook and the thread it lives on.
//!
//! The shape of this file is dictated by one constraint (DESIGN.md §9.1):
//! Windows blocks every keystroke on the machine until the hook callback
//! returns, and silently uninstalls the hook if a callback overruns
//! `LowLevelHooksTimeout`. So the callback does exactly one thing — ask the
//! Core for a Verdict, a table lookup — and posts any resulting Effect to the
//! Dispatcher over a channel. Nothing else happens on this thread.

use std::cell::RefCell;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, SetTimer,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

use super::{Health, HookWatch, vk_to_key};
use crate::{Core, Effect, KeyEvent, Verdict};

/// How long the hook may see nothing before silence becomes evidence.
const QUIET_MS: u64 = 30_000;

/// Arbitrary id for the watchdog timer; this thread owns only one.
const WATCHDOG_TIMER: usize = 1;

/// Stamped into `dwExtraInfo` on every event Footman synthesises, so the hook
/// can recognise its own work.
///
/// Filtering on this rather than on `LLKHF_INJECTED` is deliberate. That flag
/// marks *all* synthetic input, which would make Footman deaf to on-screen
/// keyboards, accessibility tools, remote desktop sessions and macro tools —
/// input that is synthetic but entirely legitimate. Only our own events need
/// excluding, and only to stop a Desktop Action's arrows (ADR-0004) being fed
/// back into the Core.
pub const FOOTMAN_SIGNATURE: usize = 0x_F007_3A11;

#[derive(Debug)]
pub struct HookError(pub String);

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The hook callback is a plain function pointer and cannot carry state, but it
/// only ever runs on the thread that installed the hook — so the state lives
/// here rather than behind a lock. A lock on this path could stall a keystroke.
struct HookState {
    core: Core,
    effects: Sender<Effect>,
    watch: HookWatch,
}

thread_local! {
    static STATE: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

/// Installs the hook and runs its message loop until the thread is asked to
/// quit. Blocks; call it on a thread of its own.
pub fn run(core: Core, effects: Sender<Effect>) -> Result<(), HookError> {
    STATE.with(|state| {
        *state.borrow_mut() = Some(HookState {
            core,
            effects,
            watch: HookWatch::new(QUIET_MS, unsafe { GetTickCount64() }),
        });
    });

    let mut hook = install()?;

    // A message loop is not optional: Windows delivers low-level hook callbacks
    // to the installing thread through it, so a thread that never pumps
    // messages never receives a single key event.
    unsafe {
        SetTimer(None, WATCHDOG_TIMER, QUIET_MS as u32, None);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            if message.message == WM_TIMER {
                hook = supervise(hook)?;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        let _ = UnhookWindowsHookEx(hook);
    }

    Ok(())
}

fn install() -> Result<HHOOK, HookError> {
    unsafe {
        // No module handle: a low-level hook's procedure lives in this process
        // rather than in a DLL injected into others, so there is nothing for
        // Windows to load.
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)
            .map_err(|e| HookError(format!("could not install the keyboard hook: {e}")))
    }
}

/// Reinstalls the hook if the evidence says Windows has dropped it.
fn supervise(hook: HHOOK) -> Result<HHOOK, HookError> {
    let now = unsafe { GetTickCount64() };
    let health = STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|s| s.watch.check(now, last_system_input()))
    });

    if health != Some(Health::Dead) {
        return Ok(hook);
    }

    unsafe {
        let _ = UnhookWindowsHookEx(hook);
    }
    let fresh = install()?;
    STATE.with(|state| {
        if let Some(s) = state.borrow_mut().as_mut() {
            s.watch.reinstalled(now);
        }
    });
    Ok(fresh)
}

/// Tick of the last input of any kind, as the system saw it.
fn last_system_input() -> u64 {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut info).as_bool() {
            u64::from(info.dwTime)
        } else {
            0
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let pass = || unsafe { CallNextHookEx(None, code, wparam, lparam) };

    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

    // Our own synthetic input must not be fed back into the Core. Anyone
    // else's synthetic input is ordinary input as far as Footman cares.
    if info.dwExtraInfo == FOOTMAN_SIGNATURE {
        return pass();
    }

    let Some(key) = vk_to_key(info.vkCode) else {
        return pass();
    };

    let event = match wparam.0 as u32 {
        WM_KEYDOWN | WM_SYSKEYDOWN => KeyEvent::Down(key),
        WM_KEYUP | WM_SYSKEYUP => KeyEvent::Up(key),
        _ => return pass(),
    };

    let verdict = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Verdict::Pass;
        };

        state.watch.saw_key(unsafe { GetTickCount64() });

        let outcome = state.core.on_event(event);
        if let Some(effect) = outcome.effect {
            // An unbounded channel, so this never blocks the hook thread. If the
            // Dispatcher has died the send fails, which must not cost a
            // keystroke — hence the discard.
            let _ = state.effects.send(effect);
        }
        outcome.verdict
    });

    match verdict {
        Verdict::Suppress => LRESULT(1),
        Verdict::Pass => pass(),
    }
}
