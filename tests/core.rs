//! Behaviour of the Core — the platform-free half of Footman.
//!
//! These exercise the public API only. The Core takes abstract key events and
//! returns a Verdict plus an optional Effect; it never touches an operating
//! system, so every rule in DESIGN.md §2.3 is verifiable here.

use footman::{
    Action, BindingTable, Chord, Core, Effect, Key, KeyEvent, Modifiers, TapAction, Verdict,
};

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

#[test]
fn hyper_pressed_and_released_alone_resolves_to_the_tap_action() {
    let mut core = Core::new(Key::CapsLock, TapAction::Escape, BindingTable::empty());

    core.on_event(KeyEvent::Down(Key::CapsLock));
    let outcome = core.on_event(KeyEvent::Up(Key::CapsLock));

    assert_eq!(outcome.verdict, Verdict::Suppress);
    assert_eq!(outcome.effect, Some(Effect::Tap(TapAction::Escape)));
}

#[test]
fn no_tap_once_a_chord_has_fired() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::Escape,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    core.on_event(KeyEvent::Down(Key::C));
    core.on_event(KeyEvent::Up(Key::C));
    let outcome = core.on_event(KeyEvent::Up(Key::CapsLock));

    assert!(outcome.effect.is_none());
}

#[test]
fn a_tap_action_of_none_produces_no_effect() {
    let mut core = core();

    core.on_event(KeyEvent::Down(Key::CapsLock));
    let outcome = core.on_event(KeyEvent::Up(Key::CapsLock));

    assert!(outcome.effect.is_none());
}

fn terminal() -> Action {
    Action::App {
        id: "aumid:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App".to_string(),
    }
}

#[test]
fn a_modifier_makes_a_different_chord() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([
            (Chord::key(Key::C), chrome()),
            (Chord::key(Key::C).with(Modifiers::SHIFT), terminal()),
        ]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    core.on_event(KeyEvent::Down(Key::Shift));
    let outcome = core.on_event(KeyEvent::Down(Key::C));

    assert_eq!(outcome.effect, Some(Effect::Run(terminal())));
}

#[test]
fn a_held_modifier_does_not_fire_the_unmodified_chord() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    core.on_event(KeyEvent::Down(Key::Shift));
    let outcome = core.on_event(KeyEvent::Down(Key::C));

    assert_eq!(outcome.verdict, Verdict::Suppress);
    assert!(outcome.effect.is_none());
}

#[test]
fn keys_pass_normally_once_hyper_is_released() {
    let mut core = Core::new(
        Key::CapsLock,
        TapAction::None,
        BindingTable::from([(Chord::key(Key::C), chrome())]),
    );

    core.on_event(KeyEvent::Down(Key::CapsLock));
    core.on_event(KeyEvent::Down(Key::C));
    core.on_event(KeyEvent::Up(Key::C));
    core.on_event(KeyEvent::Up(Key::CapsLock));

    let outcome = core.on_event(KeyEvent::Down(Key::C));

    assert_eq!(outcome.verdict, Verdict::Pass);
    assert!(outcome.effect.is_none());
}

#[test]
fn modifier_keys_themselves_always_pass() {
    let mut core = core();

    core.on_event(KeyEvent::Down(Key::CapsLock));

    for modifier in [Key::Shift, Key::Ctrl, Key::Alt] {
        assert_eq!(
            core.on_event(KeyEvent::Down(modifier)).verdict,
            Verdict::Pass
        );
        assert_eq!(core.on_event(KeyEvent::Up(modifier)).verdict, Verdict::Pass);
    }
}

/// A key whose press was swallowed must have its release swallowed too.
///
/// Windows delivers key-down and key-up as separate events, so a Core that
/// suppresses only the press leaks a lone release into the focused window.
/// Most applications ignore a release they never saw pressed, but not all —
/// and a key event Footman consumed should not be visible at all.
#[test]
fn the_release_of_a_suppressed_key_is_suppressed_too() {
    let mut core = core();
    core.on_event(KeyEvent::Down(Key::CapsLock));
    assert_eq!(
        core.on_event(KeyEvent::Down(Key::J)).verdict,
        Verdict::Suppress
    );

    let outcome = core.on_event(KeyEvent::Up(Key::J));

    assert_eq!(outcome.verdict, Verdict::Suppress);
    assert!(outcome.effect.is_none());
}

/// The release can arrive after Hyper is already up — the user lets go of both
/// in whichever order their hand happens to. The press is what decides.
#[test]
fn a_release_after_hyper_is_gone_is_still_suppressed() {
    let mut core = core();
    core.on_event(KeyEvent::Down(Key::CapsLock));
    core.on_event(KeyEvent::Down(Key::J));
    core.on_event(KeyEvent::Up(Key::CapsLock));

    assert_eq!(
        core.on_event(KeyEvent::Up(Key::J)).verdict,
        Verdict::Suppress
    );
}

/// The converse, and the more important half: a key Footman passed through must
/// have its release passed through as well, or applications see a press with no
/// release and treat the key as stuck.
#[test]
fn the_release_of_a_passed_key_passes() {
    let mut core = core();
    assert_eq!(core.on_event(KeyEvent::Down(Key::J)).verdict, Verdict::Pass);

    assert_eq!(core.on_event(KeyEvent::Up(Key::J)).verdict, Verdict::Pass);
}
