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
    Shift,
    Ctrl,
    Alt,
}

impl Key {
    /// The Modifiers bit this key contributes to a Chord, if it is a modifier.
    ///
    /// Left and right variants are not distinguished: a Chord is written
    /// `Shift+C`, never `LeftShift+C`, so the Shell normalises both sides to
    /// the same key.
    const fn as_modifier(self) -> Option<Modifiers> {
        match self {
            Key::Shift => Some(Modifiers::SHIFT),
            Key::Ctrl => Some(Modifiers::CTRL),
            Key::Alt => Some(Modifiers::ALT),
            _ => None,
        }
    }
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
    Escape,
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
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
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

    pub const fn with(self, mods: Modifiers) -> Self {
        Self { mods, ..self }
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
    Tap(TapAction),
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

    fn tap(tap: TapAction) -> Self {
        Self {
            verdict: Verdict::Suppress,
            effect: Some(Effect::Tap(tap)),
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
    tap: TapAction,
    bindings: BindingTable,
    /// Keys currently down. Two facts are read off this: whether Hyper is held,
    /// and whether a `Down` is auto-repeat rather than a new physical press.
    held: HashSet<Key>,
    /// Ordinary modifiers currently held.
    mods: Modifiers,
    /// Whether a Chord has fired since Hyper went down. This — not elapsed
    /// time — is what separates a Tap from a Chord (DESIGN.md §2.3).
    chord_fired: bool,
}

impl Core {
    pub fn new(hyper: Key, tap: TapAction, bindings: BindingTable) -> Self {
        Self {
            hyper,
            tap,
            bindings,
            held: HashSet::new(),
            mods: Modifiers::NONE,
            chord_fired: false,
        }
    }

    pub fn on_event(&mut self, event: KeyEvent) -> Outcome {
        match event {
            KeyEvent::Down(key) => self.on_down(key),
            KeyEvent::Up(key) => self.on_up(key),
        }
    }

    fn hyper_held(&self) -> bool {
        self.held.contains(&self.hyper)
    }

    fn on_down(&mut self, key: Key) -> Outcome {
        let repeat = !self.held.insert(key);

        if key == self.hyper {
            self.chord_fired = false;
            return Outcome::suppress();
        }
        if let Some(modifier) = key.as_modifier() {
            self.mods.insert(modifier);
            // A modifier alone does nothing visible, so passing it keeps the
            // OS's own modifier state in step with ours.
            return Outcome::pass();
        }
        if !self.hyper_held() {
            return Outcome::pass();
        }

        match self.bindings.get(&Chord::key(key).with(self.mods)) {
            // An Action fires once per physical press; auto-repeat is still
            // swallowed, but does nothing (DESIGN.md §2.3).
            Some(action) if !repeat => {
                let action = action.clone();
                self.chord_fired = true;
                Outcome::fire(action)
            }
            _ => Outcome::suppress(),
        }
    }

    fn on_up(&mut self, key: Key) -> Outcome {
        self.held.remove(&key);

        if let Some(modifier) = key.as_modifier() {
            self.mods.remove(modifier);
            return Outcome::pass();
        }
        if key != self.hyper {
            return Outcome::pass();
        }

        // Tap is not a duration: it is simply "Hyper came back up and no Chord
        // fired in between" (DESIGN.md §2.3).
        match self.tap {
            _ if self.chord_fired => Outcome::suppress(),
            TapAction::None => Outcome::suppress(),
            tap => Outcome::tap(tap),
        }
    }
}
