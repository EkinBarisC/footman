//! Translating a key the settings window saw into one Footman can bind.
//!
//! A Chord is recorded by pressing the key rather than by typing its name
//! (DESIGN.md §10), which means going through egui's naming. Most names agree
//! with Footman's; this is where the ones that do not are reconciled.
//!
//! It works on the name rather than on `egui::Key` so the reconciliation can be
//! tested without a window.

use std::str::FromStr;

use crate::Key;

/// The key egui calls `name`, if Footman has a name for it too.
pub fn key_named(name: &str) -> Option<Key> {
    // egui spells the arrows and digits differently from a config file, which
    // spells them the way a person writing one would.
    let name = match name {
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        digit if digit.starts_with("Num") => digit.strip_prefix("Num")?,
        other => other,
    };

    // Anything still unrecognised is refused rather than guessed at. Footman
    // has no name for punctuation on purpose: where `;` sits depends on the
    // keyboard layout.
    Key::from_str(name).ok()
}
