//! Footman's Core: the platform-free half.
//!
//! It holds the Binding table and the Hyper state machine, consumes abstract
//! key events, and returns a [`Verdict`] plus an optional [`Effect`]. It makes
//! no OS calls, reads no clock and performs no I/O — see ADR-0002.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

mod app;
mod config;
mod tray;

#[cfg(windows)]
pub mod windows;

pub use app::{AppTarget, Window, choose_window};
pub use config::{Config, ConfigError, Loaded, Warning};
pub use tray::{Click, Duty, TrayEffect};
/// Declares the key set once, and derives the enum, its parser and its display
/// from that single table so the three can never drift apart. The name in each
/// row is exactly what a Chord is written with in the config file.
macro_rules! keys {
    ($($variant:ident => $name:literal),* $(,)?) => {
        /// A key Footman can reason about, independent of any platform's key codes.
        ///
        /// Punctuation is deliberately absent: where `;` or `[` physically sits
        /// depends on the keyboard layout — the same trap that shaped ADR-0001 —
        /// so naming them would mean a config binding different keys on
        /// different machines.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Key { $($variant),* }

        impl FromStr for Key {
            type Err = ();

            fn from_str(name: &str) -> Result<Self, Self::Err> {
                $(if name.eq_ignore_ascii_case($name) { return Ok(Key::$variant); })*
                Err(())
            }
        }

        impl fmt::Display for Key {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self { $(Key::$variant => $name),* })
            }
        }

        impl Key {
            /// Every bindable key, in key-table order.
            pub const ALL: &'static [Key] = &[$(Key::$variant),*];
        }
    };
}

keys! {
    A => "A", B => "B", C => "C", D => "D", E => "E", F => "F",
    G => "G", H => "H", I => "I", J => "J", K => "K", L => "L",
    M => "M", N => "N", O => "O", P => "P", Q => "Q", R => "R",
    S => "S", T => "T", U => "U", V => "V", W => "W", X => "X",
    Y => "Y", Z => "Z",
    Digit0 => "0", Digit1 => "1", Digit2 => "2", Digit3 => "3", Digit4 => "4", Digit5 => "5",
    Digit6 => "6", Digit7 => "7", Digit8 => "8", Digit9 => "9",
    F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
    F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
    F13 => "F13", F14 => "F14", F15 => "F15", F16 => "F16", F17 => "F17", F18 => "F18",
    F19 => "F19", F20 => "F20", F21 => "F21", F22 => "F22", F23 => "F23", F24 => "F24",
    Left => "Left", Right => "Right", Up => "Up", Down => "Down",
    Space => "Space", Enter => "Enter", Tab => "Tab", Backspace => "Backspace",
    Escape => "Escape", Delete => "Delete", Insert => "Insert", Home => "Home",
    End => "End", PageUp => "PageUp", PageDown => "PageDown", CapsLock => "CapsLock",
    Shift => "Shift", Ctrl => "Ctrl", Alt => "Alt",
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

impl fmt::Display for TapAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TapAction::None => "none",
            TapAction::Escape => "escape",
        })
    }
}

impl FromStr for TapAction {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.to_ascii_lowercase().as_str() {
            "none" => Ok(TapAction::None),
            "escape" => Ok(TapAction::Escape),
            _ => Err(()),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Hyper, plus zero or more ordinary modifiers, plus exactly one key.
///
/// Hyper is implicit — every Chord begins with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl fmt::Display for Chord {
    /// Canonical spelling: modifiers in a fixed order, then the key. Chords are
    /// read case- and order-insensitively, but only ever written one way.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (modifier, name) in [
            (Modifiers::CTRL, "Ctrl"),
            (Modifiers::ALT, "Alt"),
            (Modifiers::SHIFT, "Shift"),
        ] {
            if self.mods.contains(modifier) {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", self.key)
    }
}

impl FromStr for Chord {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut parts = text.split('+').map(str::trim).rev();
        let key: Key = parts.next().ok_or(())?.parse()?;

        let mut mods = Modifiers::NONE;
        for part in parts {
            mods.insert(part.parse()?);
        }

        Ok(Chord::key(key).with(mods))
    }
}

impl FromStr for Modifiers {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.to_ascii_lowercase().as_str() {
            "shift" => Ok(Modifiers::SHIFT),
            "ctrl" => Ok(Modifiers::CTRL),
            "alt" => Ok(Modifiers::ALT),
            _ => Err(()),
        }
    }
}

/// What a Binding does when its Chord fires.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Focus an application, launching it first if it is not running.
    ///
    /// The id is opaque here — the Shell alone knows what `aumid:` or `path:`
    /// mean (ADR-0003).
    App { id: String },
    /// Hand a URL, file or folder to the operating system's default handler.
    Open { target: String },
    /// Execute a command line.
    Run {
        command: String,
        /// A console window is noise for a launcher, so it is hidden unless asked.
        #[serde(default)]
        show_window: bool,
    },
    /// Switch to a virtual desktop, numbered from 1 as the user sees them.
    Desktop { index: usize },
}

impl Action {
    /// Whether this Action could ever do what it says.
    ///
    /// Only shape is checked, never the world: whether Chrome is installed is
    /// not knowable here, and the Core must stay free of the OS (ADR-0002).
    fn check(&self) -> Result<(), String> {
        match self {
            Action::Desktop { index: 0 } => Err("desktops are numbered from 1".to_string()),
            _ => Ok(()),
        }
    }
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
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BindingTable {
    bindings: HashMap<Chord, Action>,
}

impl BindingTable {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, chord: &Chord) -> Option<&Action> {
        self.bindings.get(chord)
    }

    /// Every Binding, in a fixed order — key-table order, not map order — so
    /// that writing the config file produces the same bytes every time.
    pub fn sorted(&self) -> Vec<(Chord, &Action)> {
        let mut all: Vec<_> = self.bindings.iter().map(|(&c, a)| (c, a)).collect();
        all.sort_by_key(|(chord, _)| *chord);
        all
    }

    /// Adds a Binding, keeping any Binding already on that Chord.
    ///
    /// Returns whether the Chord was free. The first Binding wins so that a
    /// duplicate can be reported rather than silently overwriting (DESIGN.md §7).
    pub fn add(&mut self, chord: Chord, action: Action) -> bool {
        match self.bindings.entry(chord) {
            Entry::Occupied(_) => false,
            Entry::Vacant(slot) => {
                slot.insert(action);
                true
            }
        }
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
    /// Keys whose press was suppressed, so their release can be suppressed too.
    /// Keyed on the press because that is when the decision is made: by the
    /// time the release arrives, Hyper may already be up (DESIGN.md §2.3).
    swallowed: HashSet<Key>,
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
            swallowed: HashSet::new(),
            mods: Modifiers::NONE,
            chord_fired: false,
        }
    }

    /// Drops everything the Core believed about the keyboard.
    ///
    /// Called when Footman stops or starts watching (DESIGN.md §10). While it
    /// is paused the hook is uninstalled, so keys go down and come up unseen:
    /// anything remembered from before is not merely stale but wrong, and
    /// acting on it would swallow a release whose press the application already
    /// received.
    pub fn forget(&mut self) {
        self.held.clear();
        self.swallowed.clear();
        self.mods = Modifiers::NONE;
        self.chord_fired = false;
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
            self.swallowed.insert(key);
            return Outcome::suppress();
        }
        if let Some(modifier) = key.as_modifier() {
            self.mods.insert(modifier);
            // A modifier alone does nothing visible, so passing it keeps the
            // OS's own modifier state in step with ours.
            return Outcome::pass();
        }
        if !self.hyper_held() {
            // This press reaches the application, so its release must too —
            // even if an earlier press of the same key, under Hyper, did not.
            self.swallowed.remove(&key);
            return Outcome::pass();
        }

        // Whatever the lookup says, this press is Footman's; its release is
        // Footman's too.
        self.swallowed.insert(key);

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
        let swallowed = self.swallowed.remove(&key);

        if let Some(modifier) = key.as_modifier() {
            self.mods.remove(modifier);
            return Outcome::pass();
        }
        if key != self.hyper {
            return if swallowed {
                Outcome::suppress()
            } else {
                Outcome::pass()
            };
        }

        // A release whose press Footman never saw — the key was already down
        // when the hook was installed, or went down while Footman was paused.
        // The application has the press; suppressing the release would strand
        // the key held forever.
        if !swallowed {
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
