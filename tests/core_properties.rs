//! The Core's safety invariants, stated as properties.
//!
//! These are universal claims — "*never* suppress unless Hyper is held" — so
//! examples cannot establish them. They correspond to the governing rule in
//! DESIGN.md §7: the keyboard comes first.

use footman::{Action, BindingTable, Chord, Core, Effect, Key, KeyEvent, TapAction, Verdict};
use proptest::prelude::*;

const HYPER: Key = Key::CapsLock;

/// Every key except the Hyper Key, which the properties drive explicitly.
const ORDINARY: [Key; 5] = [Key::C, Key::J, Key::Shift, Key::Ctrl, Key::Alt];

fn ordinary_key() -> impl Strategy<Value = Key> {
    prop::sample::select(ORDINARY.as_slice())
}

fn event() -> impl Strategy<Value = KeyEvent> {
    (ordinary_key(), any::<bool>()).prop_map(|(key, down)| {
        if down {
            KeyEvent::Down(key)
        } else {
            KeyEvent::Up(key)
        }
    })
}

/// Hyper included, so the properties see it interleave with everything else.
fn hyper_or_ordinary() -> impl Strategy<Value = KeyEvent> {
    (
        prop::sample::select([Key::C, Key::J, Key::Shift, Key::Ctrl, HYPER].as_slice()),
        any::<bool>(),
    )
        .prop_map(|(key, down)| {
            if down {
                KeyEvent::Down(key)
            } else {
                KeyEvent::Up(key)
            }
        })
}

fn chrome() -> Action {
    Action::App {
        id: "aumid:Chrome".to_string(),
    }
}

proptest! {
    /// The fail-open guarantee. If Hyper is never pressed, Footman is invisible:
    /// it must not swallow a single key, whatever the user types.
    #[test]
    fn nothing_is_suppressed_while_hyper_is_untouched(events in prop::collection::vec(event(), 0..200)) {
        let mut core = Core::new(
            HYPER,
            TapAction::Escape,
            BindingTable::from([(Chord::key(Key::C), chrome())]),
        );

        for event in events {
            let outcome = core.on_event(event);
            prop_assert_eq!(outcome.verdict, Verdict::Pass, "suppressed {:?} with Hyper untouched", event);
            prop_assert!(outcome.effect.is_none(), "fired on {:?} with Hyper untouched", event);
        }
    }

    /// With nothing bound, holding Hyper can never cause anything to happen —
    /// it can only swallow. An empty configuration is inert, not dangerous.
    #[test]
    fn an_empty_binding_table_can_never_fire(events in prop::collection::vec(event(), 0..200)) {
        let mut core = Core::new(HYPER, TapAction::None, BindingTable::empty());

        core.on_event(KeyEvent::Down(HYPER));
        for event in events {
            prop_assert!(core.on_event(event).effect.is_none(), "fired on {:?} with no bindings", event);
        }
    }

    /// However long a bound key is held, the Action runs exactly once. The
    /// alternative is launching an application thirty times a second.
    #[test]
    fn any_number_of_repeats_fires_exactly_once(repeats in 0usize..500) {
        let mut core = Core::new(
            HYPER,
            TapAction::None,
            BindingTable::from([(Chord::key(Key::C), chrome())]),
        );

        core.on_event(KeyEvent::Down(HYPER));

        let fired = std::iter::once(core.on_event(KeyEvent::Down(Key::C)))
            .chain((0..repeats).map(|_| core.on_event(KeyEvent::Down(Key::C))))
            .filter(|outcome| outcome.effect == Some(Effect::Run(chrome())))
            .count();

        prop_assert_eq!(fired, 1);
    }

    /// No key is ever left stuck, in either direction. An application that saw
    /// a press must see the matching release, or the key is held down forever;
    /// and a press Footman swallowed must not be followed by a release the
    /// application never had a press for. Whatever order Hyper and the key are
    /// pressed and let go in.
    #[test]
    fn presses_and_releases_are_decided_alike(events in prop::collection::vec(hyper_or_ordinary(), 0..200)) {
        let mut core = Core::new(
            HYPER,
            TapAction::Escape,
            BindingTable::from([(Chord::key(Key::C), chrome())]),
        );
        // What the application below Footman believes about each key.
        let mut believed_pressed = std::collections::HashSet::new();
        let mut swallowed_press = std::collections::HashSet::new();

        for event in events {
            let passed = core.on_event(event).verdict == Verdict::Pass;
            match event {
                KeyEvent::Down(key) => {
                    if passed {
                        believed_pressed.insert(key);
                        swallowed_press.remove(&key);
                    } else {
                        swallowed_press.insert(key);
                        believed_pressed.remove(&key);
                    }
                }
                KeyEvent::Up(key) => {
                    if believed_pressed.remove(&key) {
                        prop_assert!(passed, "suppressed the release of {:?}, leaving it stuck down", key);
                    } else if swallowed_press.remove(&key) {
                        prop_assert!(!passed, "passed the release of {:?}, whose press was swallowed", key);
                    }
                    // A release with no press at all — the key was already down
                    // when Footman started — is nobody's business either way.
                }
            }
        }
    }
}
