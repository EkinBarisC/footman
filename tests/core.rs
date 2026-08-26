//! Behaviour of the Core — the platform-free half of Footman.
//!
//! These exercise the public API only. The Core takes abstract key events and
//! returns a Verdict plus an optional Effect; it never touches an operating
//! system, so every rule in DESIGN.md §2.3 is verifiable here.

use footman::{Action, BindingTable, Chord, Core, Effect, Key, KeyEvent, TapAction, Verdict};

fn core() -> Core {
    Core::new(Key::CapsLock, TapAction::None, BindingTable::empty())
}

#[test]
fn ordinary_keys_pass_when_hyper_is_not_held() {
    let mut core = core();

    let outcome = core.on_event(KeyEvent::Down(Key::C));

    assert_eq!(outcome.verdict, Verdict::Pass);
    assert!(outcome.effect.is_none());
}

#[test]
fn the_hyper_key_itself_is_suppressed() {
    let mut core = core();

    let outcome = core.on_event(KeyEvent::Down(Key::CapsLock));

    assert_eq!(outcome.verdict, Verdict::Suppress);
}

fn chrome() -> Action {
    Action::App {
        id: "aumid:Chrome".to_string(),
    }
}

#[test]
fn a_bound_chord_is_suppressed_and_fires_its_action() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    let outcome = core.on_event(KeyEvent::Down(Key::C));

    assert_eq!(outcome.verdict, Verdict::Suppress);
    assert_eq!(outcome.effect, Some(Effect::Run(chrome())));
}

#[test]
fn an_unbound_key_under_hyper_is_suppressed_and_does_nothing() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    let outcome = core.on_event(KeyEvent::Down(Key::J));

    assert_eq!(outcome.verdict, Verdict::Suppress);
    assert!(outcome.effect.is_none());
}

#[test]
fn holding_a_bound_key_fires_exactly_once() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));

    let first = core.on_event(KeyEvent::Down(Key::C));
    let repeats: Vec<_> = (0..30)
        .map(|_| core.on_event(KeyEvent::Down(Key::C)))
        .collect();

    assert_eq!(first.effect, Some(Effect::Run(chrome())));
    assert!(
        repeats.iter().all(|o| o.effect.is_none()),
        "auto-repeat must not re-fire the Action"
    );
    assert!(
        repeats.iter().all(|o| o.verdict == Verdict::Suppress),
        "repeats are still swallowed"
    );
}

#[test]
fn releasing_and_pressing_again_fires_again() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    core.on_event(KeyEvent::Down(Key::C));
    core.on_event(KeyEvent::Up(Key::C));
    let second = core.on_event(KeyEvent::Down(Key::C));

    assert_eq!(second.effect, Some(Effect::Run(chrome())));
}
