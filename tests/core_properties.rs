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
}
