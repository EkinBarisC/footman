//! Reading a Chord from a key the user pressed in the settings window.
//!
//! A Chord is recorded by pressing the key, never by typing its name
//! (DESIGN.md §10) — so the settings window has to translate egui's names into
//! Footman's. Most agree; the ones that do not are the point of these tests,
//! and are the ones that would otherwise be discovered by a user finding that
//! the arrow keys cannot be bound.

#![cfg(windows)]

use footman::Key;
use footman::windows::key_named;

#[test]
fn names_that_already_agree_are_taken_as_they_are() {
    assert_eq!(key_named("A"), Some(Key::A));
    assert_eq!(key_named("F7"), Some(Key::F7));
    assert_eq!(key_named("Escape"), Some(Key::Escape));
    assert_eq!(key_named("PageDown"), Some(Key::PageDown));
}

/// egui calls them `ArrowLeft`; a config file calls them `Left`, because that
/// is what a person writing one would type.
#[test]
fn the_arrows_are_translated() {
    assert_eq!(key_named("ArrowLeft"), Some(Key::Left));
    assert_eq!(key_named("ArrowRight"), Some(Key::Right));
    assert_eq!(key_named("ArrowUp"), Some(Key::Up));
    assert_eq!(key_named("ArrowDown"), Some(Key::Down));
}

/// egui calls them `Num0`; a config file calls them `0`.
#[test]
fn the_digits_are_translated() {
    assert_eq!(key_named("Num0"), Some(Key::Digit0));
    assert_eq!(key_named("Num7"), Some(Key::Digit7));
}

/// A key Footman has no name for — punctuation, deliberately absent because
/// where `;` sits depends on the keyboard layout. Refused rather than guessed,
/// so capture simply does not record it.
#[test]
fn a_key_footman_does_not_name_is_refused() {
    assert_eq!(key_named("Semicolon"), None);
    assert_eq!(key_named("Backslash"), None);
    assert_eq!(key_named(""), None);
}
