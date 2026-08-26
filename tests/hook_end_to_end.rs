//! The end-to-end check DESIGN.md §12 asks of slice 3: a real hook, real input.
//!
//! Opt-in twice over — behind a feature *and* `#[ignore]`:
//!
//! ```text
//! cargo test --features integration-test --test hook_end_to_end -- --ignored --test-threads=1
//! ```
//!
//! It installs a system-wide keyboard hook and synthesises real keystrokes.
//! Every key it sends is one Footman suppresses, so nothing leaks into the
//! focused window, but it is not something to run absent-mindedly.
//!
//! **It does not pass on every machine, and a failure here is not necessarily a
//! failure of Footman.** On the development machine these tests see zero hook
//! callbacks even though `SendInput` reports success — and so does a bare
//! reference hook that uses no Footman code at all. The likeliest cause is UIPI
//! silently discarding injected input while a higher-privilege window (a
//! uiAccess helper such as Raycast's) holds the foreground. Until that is
//! settled, slice 3 is verified by running the binary and pressing keys.
//!
//! `--test-threads=1` is required: each test installs its own hook, and
//! concurrent hooks in one process see each other's events.

#![cfg(all(windows, feature = "integration-test"))]

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use footman::windows::test_support::{send, tap};
use footman::{Action, BindingTable, Chord, Core, Effect, Key, KeyEvent, TapAction};

fn chrome() -> Action {
    Action::App {
        id: "aumid:Chrome".to_string(),
    }
}

/// Long enough for the hook thread to install and for Windows to deliver.
const SETTLE: Duration = Duration::from_millis(400);

/// The Hyper Key for these tests.
///
/// Deliberately not Caps Lock. A developer's machine very often already runs
/// something that owns Caps Lock — Raycast, PowerToys, an AutoHotkey script —
/// and a hook earlier in the chain that suppresses it would make these tests
/// fail for reasons that have nothing to do with Footman. F24 is on no
/// keyboard anyone owns and no tool claims it.
const HYPER: Key = Key::F24;

/// Likewise: a key nothing else is watching for.
const BOUND: Key = Key::F23;
const UNBOUND: Key = Key::F22;

#[test]
#[ignore = "needs an environment where SendInput reaches low-level hooks; see the module comment"]
fn real_keystrokes_reach_the_core_and_produce_an_effect() {
    let (effects, inbox) = mpsc::channel::<Effect>();

    let mut table = BindingTable::empty();
    table.add(Chord::key(BOUND), chrome());
    let core = Core::new(HYPER, TapAction::None, table);

    thread::spawn(move || {
        footman::windows::run(core, effects).expect("the hook must install");
    });
    thread::sleep(SETTLE);

    // Hyper down, C, Hyper up — typed as a human would.
    send(HYPER, true);
    tap(BOUND);
    send(HYPER, false);

    let effect = inbox
        .recv_timeout(Duration::from_secs(2))
        .expect("the Chord must reach the Dispatcher");
    assert_eq!(effect, Effect::Run(chrome()));

    // An unbound key under Hyper is swallowed and produces nothing.
    send(HYPER, true);
    tap(UNBOUND);
    send(HYPER, false);

    assert!(
        inbox.recv_timeout(SETTLE).is_err(),
        "an unbound key must produce no Effect"
    );
}

/// Guards the accessibility decision in `hook.rs`: Footman filters only input it
/// stamped itself, not everything Windows marks as injected. On-screen
/// keyboards, remote desktop and accessibility tools all send injected input,
/// and Footman has to answer to them.
///
/// The whole test would be impossible if that were wrong — `SendInput` produces
/// injected events — so this asserts it explicitly rather than by accident.
#[test]
#[ignore = "needs an environment where SendInput reaches low-level hooks; see the module comment"]
fn injected_input_from_elsewhere_is_treated_as_real() {
    let (effects, inbox) = mpsc::channel::<Effect>();

    let mut table = BindingTable::empty();
    table.add(Chord::key(BOUND), chrome());
    let mut core = Core::new(HYPER, TapAction::None, table);
    // Prove the Core agrees before any hook is involved.
    core.on_event(KeyEvent::Down(HYPER));
    assert!(core.on_event(KeyEvent::Down(BOUND)).effect.is_some());
    core.on_event(KeyEvent::Up(BOUND));
    core.on_event(KeyEvent::Up(HYPER));

    thread::spawn(move || {
        footman::windows::run(core, effects).expect("the hook must install");
    });
    thread::sleep(SETTLE);

    send(HYPER, true);
    tap(BOUND);
    send(HYPER, false);

    assert!(
        inbox.recv_timeout(Duration::from_secs(2)).is_ok(),
        "synthetic input that is not Footman's own must be treated as real"
    );
}
