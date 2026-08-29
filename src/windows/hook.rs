//! The low-level keyboard hook and the thread it lives on.
//!
//! The shape of this file is dictated by one constraint (DESIGN.md §9.1):
//! Windows blocks every keystroke on the machine until the hook callback
//! returns, and silently uninstalls the hook if a callback overruns
//! `LowLevelHooksTimeout`. So the callback does exactly one thing — ask the
//! Core for a Verdict, a table lookup — and posts any resulting Effect to the
//! Dispatcher over a channel. Nothing else happens on this thread.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{LazyLock, Mutex};
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

use super::{Health, HookWatch, same_clock, vk_to_key};
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

/// The hook callback is a plain function pointer and cannot carry state, so the
/// state lives beside it.
///
/// Behind a lock, and deliberately: **the callback does not always run on the
/// thread that installed the hook.** While a window of this process holds the
/// keyboard focus, Windows runs it on that window's thread instead — measured,
/// not read (ADR-0007). Kept in a `thread_local!` the state was simply absent
/// on that thread, and every Chord pressed with the settings window focused
/// fell through as though Footman were not running.
///
/// The lock is held for a table lookup and a channel send, and the only threads
/// that contend for it are running this same callback — which Windows
/// serialises anyway, being blocked on each answer before it delivers the next
/// key.
struct HookState {
    core: Core,
    effects: Sender<Effect>,
    watch: HookWatch,
    /// Replacement Cores from the settings window. Read here rather than only
    /// in the message loop so that a saved Binding is live from the next
    /// keystroke, whatever else the loop is or is not being given time to do.
    fresh: Receiver<Core>,
    /// Where trace lines go when `FOOTMAN_TRACE` is set. A channel rather than
    /// a `println!` because writing to a console from the callback is exactly
    /// the kind of unbounded wait that gets a hook uninstalled (DESIGN.md
    /// §9.1) — the line is printed by somebody else's thread.
    trace: Option<Sender<String>>,
}

impl HookState {
    /// Takes a replacement Core if one is waiting. Cheap enough to sit on the
    /// hot path: an empty channel is an atomic read.
    fn adopt(&mut self) {
        while let Ok(core) = self.fresh.try_recv() {
            self.core = core;
        }
    }
}

static STATE: LazyLock<Mutex<Option<HookState>>> = LazyLock::new(|| Mutex::new(None));

/// Reaches the hook's state, from whichever thread the callback landed on.
///
/// The guard is released before this returns, so nothing that takes the lock
/// may be called from inside `body` — `say` in particular.
fn with_state<T>(body: impl FnOnce(&mut Option<HookState>) -> T) -> T {
    // A panic elsewhere must not take the keyboard with it: a poisoned lock
    // still holds a perfectly good Core.
    let mut state = STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    body(&mut state)
}

/// A handle on the running hook thread, from any other thread.
///
/// Everything it can ask for is a message posted to that thread rather than a
/// lock taken on shared state: the hook thread must never wait on anything, and
/// a mutex on this path could stall a keystroke.
pub struct Hook {
    thread: u32,
    /// Replacement Cores, waiting for the hook thread to pick them up. A
    /// message cannot carry one, and a lock would put the settings window on
    /// the path of every keystroke.
    cores: Sender<Core>,
}

/// Stand down: uninstall the hook and leave the keyboard entirely normal.
const PAUSE: u32 = WM_APP + 1;
/// Install it again.
const RESUME: u32 = WM_APP + 2;
/// Take the Core waiting on the channel — the settings were saved.
const REBIND: u32 = WM_APP + 3;

impl Hook {
    pub fn pause(&self) {
        self.post(PAUSE);
    }

    pub fn resume(&self) {
        self.post(RESUME);
    }

    /// Start using a new set of Bindings, without dropping the hook: the user
    /// pressing Save should not have their keyboard flicker.
    pub fn rebind(&self, core: Core) {
        if self.cores.send(core).is_ok() {
            self.post(REBIND);
        }
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
    let (cores, fresh) = mpsc::channel();

    thread::spawn(move || {
        let thread = unsafe { GetCurrentThreadId() };
        if tell.send(thread).is_err() {
            return;
        }
        run(core, effects, duty, wake, fresh);
    });

    heard
        .recv()
        .map(|thread| Hook { thread, cores })
        .map_err(|_| HookError("the hook thread stopped before it started".to_string()))
}

/// Installs the hook and runs its message loop until asked to quit.
fn run(core: Core, effects: Sender<Effect>, duty: Sender<Duty>, wake: u32, fresh: Receiver<Core>) {
    with_state(|state| {
        *state = Some(HookState {
            core,
            effects,
            watch: HookWatch::new(QUIET_MS, unsafe { GetTickCount64() }),
            fresh,
            trace: trace_channel(),
        });
    });

    let mut hook = start(&duty, wake);

    // A message loop is not optional: Windows delivers low-level hook callbacks
    // through one, so a thread that never pumps messages never receives a
    // single key event. Which loop it uses is not ours to choose — see
    // `HookState` — but this one has to exist, and it is where the watchdog and
    // everything the settings window asks for get answered.
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
                // A nudge rather than the delivery itself: the Core is picked
                // up here if the loop is free, and by the callback if it is
                // not. Either way the next keystroke sees the new Bindings.
                REBIND => with_state(|state| {
                    if let Some(state) = state.as_mut() {
                        state.adopt();
                    }
                }),
                _ => {}
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        stop(hook);
    }
}

/// A printer for trace lines, on a thread of its own, if `FOOTMAN_TRACE` is
/// set in the environment. Off by default: this reports every keystroke on the
/// machine, which is not something to leave running.
fn trace_channel() -> Option<Sender<String>> {
    std::env::var_os("FOOTMAN_TRACE")?;

    let (say, said) = mpsc::channel::<String>();
    thread::spawn(move || {
        for line in said {
            println!("footman: {line}");
        }
    });
    Some(say)
}

/// Installs the hook and says so. The Core forgets first: whatever it believed
/// about the keyboard from before is worthless, because keys went down and came
/// up unseen while the hook was gone.
fn start(duty: &Sender<Duty>, wake: u32) -> Option<HHOOK> {
    with_state(|state| {
        if let Some(state) = state.as_mut() {
            state.core.forget();
            state.watch.reinstalled(unsafe { GetTickCount64() });
        }
    });

    match install() {
        Ok(hook) => {
            say("hook installed");
            report(duty, wake, Duty::Active);
            Some(hook)
        }
        Err(error) => {
            say(format!("hook could not be installed: {error}"));
            report(duty, wake, Duty::Broken);
            None
        }
    }
}

/// Says something on the trace, if there is one. Called only from this thread.
fn say(line: impl Into<String>) {
    // The sender is taken out rather than used in place: sending while holding
    // the lock would put a console write on the path of the next keystroke.
    let trace = with_state(|state| state.as_ref().and_then(|state| state.trace.clone()));
    if let Some(trace) = trace {
        let _ = trace.send(line.into());
    }
}

fn stop(hook: Option<HHOOK>) -> Option<HHOOK> {
    if let Some(hook) = hook {
        say("hook uninstalled");
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
    let health = with_state(|state| {
        state
            .as_ref()
            .map(|state| state.watch.check(now, last_system_input(now)))
    });

    if health != Some(Health::Dead) {
        return hook;
    }

    say("hook looked dead; reinstalling");
    stop(hook);
    // A reinstall that fails leaves nothing installed and the watchdog will try
    // again on the next tick. It does not report Broken: an unattended retry is
    // not news, and the tray flickering between states would be.
    let fresh = install().ok();
    with_state(|state| {
        if let Some(state) = state.as_mut() {
            state.watch.reinstalled(now);
        }
    });
    fresh
}

/// Tick of the last input of any kind, as the system saw it, counted on the
/// same clock as everything else here.
fn last_system_input(now: u64) -> u64 {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut info).as_bool() {
            same_clock(now, info.dwTime)
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

    let verdict = with_state(|state| {
        let Some(state) = state.as_mut() else {
            return Verdict::Pass;
        };

        state.watch.saw_key(unsafe { GetTickCount64() });
        state.adopt();

        let outcome = state.core.on_event(event);
        if let Some(trace) = &state.trace {
            let _ = trace.send(format!(
                "{event:?} -> {:?} effect={:?} {}",
                outcome.verdict,
                outcome.effect,
                state.core.believes()
            ));
        }
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
