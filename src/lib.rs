//! Footman's Core: the platform-free half.
//!
//! It holds the Binding table and the Hyper state machine, consumes abstract
//! key events, and returns a [`Verdict`] plus an optional [`Effect`]. It makes
//! no OS calls, reads no clock and performs no I/O — see ADR-0002.

use std::collections::{HashMap, HashSet};
/// A key Footman can reason about, independent of any platform's key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    C,
    J,
    CapsLock,
}

/// One raw key transition, as reported by the Shell.
///
/// There is no auto-repeat flag: Windows' low-level hook does not provide one,
/// so the Core derives repeats itself from a second `Down` without an
/// intervening `Up`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Down(Key),
    Up(Key),
}

/// What happens when the Hyper Key is released without any Chord having fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapAction {
    None,
}

/// The Core's ruling on one key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The system sees the event untouched.
    Pass,
    /// The event is dropped and never reaches the foreground app.
    Suppress,
}

/// Ordinary modifiers that may accompany Hyper in a Chord.
///
/// Win is deliberately absent: unsuppressed it opens the Start menu, and
/// suppressing it would mean synthesising input (ADR-0001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
}

/// Hyper, plus zero or more ordinary modifiers, plus exactly one key.
///
/// Hyper is implicit — every Chord begins with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: Modifiers,
    pub key: Key,
}

impl Chord {
    pub const fn key(key: Key) -> Self {
        Self {
            mods: Modifiers::NONE,
            key,
        }
    }
}

/// What a Binding does when its Chord fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Focus an application, launching it first if it is not running.
    App { id: String },
}

/// Something for the Dispatcher to carry out, alongside the Verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Run(Action),
}

/// The Core's full reply to one key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub verdict: Verdict,
    pub effect: Option<Effect>,
}

impl Outcome {
    fn pass() -> Self {
        Self {
            verdict: Verdict::Pass,
            effect: None,
        }
    }

    fn suppress() -> Self {
        Self {
            verdict: Verdict::Suppress,
            effect: None,
        }
    }

    fn fire(action: Action) -> Self {
        Self {
            verdict: Verdict::Suppress,
            effect: Some(Effect::Run(action)),
        }
    }
}

/// The set of Bindings in force.
#[derive(Debug, Default)]
pub struct BindingTable {
    bindings: HashMap<Chord, Action>,
}

impl BindingTable {
    pub fn empty() -> Self {
        Self::default()
    }

    fn get(&self, chord: &Chord) -> Option<&Action> {
        self.bindings.get(chord)
    }
}

impl<const N: usize> From<[(Chord, Action); N]> for BindingTable {
    fn from(bindings: [(Chord, Action); N]) -> Self {
        Self {
            bindings: HashMap::from(bindings),
        }
    }
}

/// The Hyper state machine and Binding table.
#[derive(Debug)]
pub struct Core {
    hyper: Key,
    _tap: TapAction,
    bindings: BindingTable,
    hyper_held: bool,
    /// Keys currently down. A `Down` for a key already in here is auto-repeat,
    /// not a new physical press, and must not fire an Action a second time.
    held: HashSet<Key>,
}

impl Core {
    pub fn new(hyper: Key, tap: TapAction, bindings: BindingTable) -> Self {
        Self {
            hyper,
            _tap: tap,
            bindings,
            hyper_held: false,
            held: HashSet::new(),
        }
    }

    pub fn on_event(&mut self, event: KeyEvent) -> Outcome {
        match event {
            KeyEvent::Down(key) => {
                let repeat = !self.held.insert(key);

                if key == self.hyper {
                    self.hyper_held = true;
                    return Outcome::suppress();
                }
                if !self.hyper_held {
                    return Outcome::pass();
                }
                match self.bindings.get(&Chord::key(key)) {
                    // An Action fires once per physical press; auto-repeat is
                    // still swallowed, but does nothing (DESIGN.md §2.3).
                    Some(action) if !repeat => Outcome::fire(action.clone()),
                    _ => Outcome::suppress(),
                }
            }
            KeyEvent::Up(key) => {
                self.held.remove(&key);

                if key == self.hyper {
                    self.hyper_held = false;
                    return Outcome::suppress();
                }
                Outcome::pass()
            }
        }
    }
}
