//! The one hard performance constraint in Footman.
//!
//! Windows uninstalls a low-level keyboard hook whose callback overruns
//! `LowLevelHooksTimeout` — 300 ms by default — and reports nothing
//! (DESIGN.md §9.1). Every keystroke on the machine waits on that callback, so
//! the decision has to be trivially cheap and everything else has to happen
//! somewhere else.
//!
//! This measures the decision alone. It is not a benchmark: the assertion is
//! deliberately three orders of magnitude looser than the real limit, so it
//! fails only if something genuinely expensive has been put on the hot path.

use std::time::Instant;

use footman::{Action, BindingTable, Chord, Core, Key, KeyEvent, TapAction};

/// The real limit is 300 ms per event. Anything approaching a millisecond means
/// I/O or allocation has crept into the decision.
const BUDGET_MICROS: f64 = 100.0;

#[test]
fn deciding_a_key_event_stays_far_inside_the_hook_timeout() {
    let bindings: Vec<_> = Key::ALL
        .iter()
        .map(|&key| {
            (
                Chord::key(key),
                Action::App {
                    id: format!("aumid:{key}"),
                },
            )
        })
        .collect();

    let mut table = BindingTable::empty();
    for (chord, action) in bindings {
        table.add(chord, action);
    }

    let mut core = Core::new(Key::CapsLock, TapAction::None, table);
    core.on_event(KeyEvent::Down(Key::CapsLock));

    // Alternating down/up so both paths, and the Binding lookup, are exercised.
    let rounds = 100_000;
    let started = Instant::now();
    for _ in 0..rounds {
        core.on_event(KeyEvent::Down(Key::C));
        core.on_event(KeyEvent::Up(Key::C));
    }
    let per_event = started.elapsed().as_secs_f64() * 1e6 / (rounds * 2) as f64;

    assert!(
        per_event < BUDGET_MICROS,
        "{per_event:.2}µs per event; the hook callback budget is 300000µs and \
         anything near it means work belongs on the Dispatcher instead"
    );
}
