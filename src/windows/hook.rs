//! The low-level keyboard hook and the thread it lives on.
//!
//! The shape of this file is dictated by one constraint (DESIGN.md §9.1):
//! Windows blocks every keystroke on the machine until the hook callback
//! returns, and silently uninstalls the hook if a callback overruns
//! `LowLevelHooksTimeout`. So the callback does exactly one thing — ask the
//! Core for a Verdict, a table lookup — and posts any resulting Effect to the
//! Dispatcher over a channel. Nothing else happens on this thread.

use std::cell::RefCell;
use std::sync::mpsc::{self, Sender};
use std::thread;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, PostThreadMessageW,
    SetTimer, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_APP,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

use super::{Health, HookWatch, vk_to_key};
use crate::{Core, Duty, Effect, KeyEvent, Verdict};

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

/// A handle on the running hook thread, from any other thread.
///
/// Everything it can ask for is a message posted to that thread rather than a
/// lock taken on shared state: the hook thread must never wait on anything, and
/// a mutex on this path could stall a keystroke.
pub struct Hook {
    thread: u32,
}

/// Stand down: uninstall the hook and leave the keyboard entirely normal.
const PAUSE: u32 = WM_APP + 1;
/// Install it again.
const RESUME: u32 = WM_APP + 2;

impl Hook {
    pub fn pause(&self) {
        self.post(PAUSE);
    }

    pub fn resume(&self) {
        self.post(RESUME);
    }

    /// Ends the message loop, which uninstalls the hook on its way out.
    pub fn quit(&self) {
        self.post(WM_QUIT);
    }

    fn post(&self, message: u32) {
        unsafe {
            let _ = PostThreadMessageW(self.thread, message, WPARAM(0), LPARAM(0));
        }
    }
}

/// Starts the hook on a thread of its own and returns a handle to it.
///
/// `duty` receives every change of state, including the first: whether the hook
/// installed at all is reported there rather than returned, because a Footman
/// that could not install must stay alive to be retried (DESIGN.md §7).
/// `wake` is the thread to nudge when that happens, so a tray sitting in
/// `GetMessage` notices.
pub fn spawn(
    core: Core,
    effects: Sender<Effect>,
    duty: Sender<Duty>,
    wake: u32,
) -> Result<Hook, HookError> {
    let (tell, heard) = mpsc::channel();

    thread::spawn(move || {
        let thread = unsafe { GetCurrentThreadId() };
        if tell.send(thread).is_err() {
            return;
        }
        run(core, effects, duty, wake);
    });

    heard
        .recv()
        .map(|thread| Hook { thread })
        .map_err(|_| HookError("the hook thread stopped before it started".to_string()))
}

/// Installs the hook and runs its message loop until asked to quit.
fn run(core: Core, effects: Sender<Effect>, duty: Sender<Duty>, wake: u32) {
    STATE.with(|state| {
        *state.borrow_mut() = Some(HookState {
            core,
            effects,
            watch: HookWatch::new(QUIET_MS, unsafe { GetTickCount64() }),
        });
    });

    let mut hook = start(&duty, wake);

    // A message loop is not optional: Windows delivers low-level hook callbacks
    // to the installing thread through it, so a thread that never pumps
    // messages never receives a single key event.
    unsafe {
        SetTimer(None, WATCHDOG_TIMER, QUIET_MS as u32, None);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            match message.message {
                // Only while working. A paused Footman is *meant* to see
                // nothing, and the watchdog reads silence as death — left to
                // itself it would reinstall the hook the user just asked to be
                // rid of.
                WM_TIMER if hook.is_some() => hook = supervise(hook),
                PAUSE => {
                    hook = stop(hook);
                    report(&duty, wake, Duty::Paused);
                }
                RESUME if hook.is_none() => hook = start(&duty, wake),
                _ => {}
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        stop(hook);
    }
}

/// Installs the hook and says so. The Core forgets first: whatever it believed
/// about the keyboard from before is worthless, because keys went down and came
/// up unseen while the hook was gone.
fn start(duty: &Sender<Duty>, wake: u32) -> Option<HHOOK> {
    STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            state.core.forget();
            state.watch.reinstalled(unsafe { GetTickCount64() });
        }
    });

    match install() {
        Ok(hook) => {
            report(duty, wake, Duty::Active);
            Some(hook)
        }
        Err(_) => {
            report(duty, wake, Duty::Broken);
            None
        }
    }
}

fn stop(hook: Option<HHOOK>) -> Option<HHOOK> {
    if let Some(hook) = hook {
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    None
}

fn report(duty: &Sender<Duty>, wake: u32, state: Duty) {
    let _ = duty.send(state);
    // A tray blocked in GetMessage would not look at the channel until
    // something else happened to arrive.
    unsafe {
        let _ = PostThreadMessageW(wake, WM_APP, WPARAM(0), LPARAM(0));
    }
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
fn supervise(hook: Option<HHOOK>) -> Option<HHOOK> {
    let now = unsafe { GetTickCount64() };
    let health = STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|s| s.watch.check(now, last_system_input()))
    });

    if health != Some(Health::Dead) {
        return hook;
    }

    stop(hook);
    // A reinstall that fails leaves nothing installed and the watchdog will try
    // again on the next tick. It does not report Broken: an unattended retry is
    // not news, and the tray flickering between states would be.
    let fresh = install().ok();
    STATE.with(|state| {
        if let Some(s) = state.borrow_mut().as_mut() {
            s.watch.reinstalled(now);
        }
    });
    fresh
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
