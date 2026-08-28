//! Which windows Footman is willing to see.
//!
//! Windows keeps a great many top-level windows alive that no user would call a
//! window: suspended UWP applications, the touch-keyboard host, the inner half
//! of every UWP application's split window. They pass every classic Alt-Tab
//! test — visible, titled, unowned, not a tool window — and are separated from
//! real windows only by DWM cloaking.
//!
//! The rule below was measured rather than guessed, on Windows 11:
//!
//! | Window | cloaked | on this desktop |
//! | --- | --- | --- |
//! | Chrome, Steam (really open, here) | 0 | yes |
//! | Claude, Unity Hub (really open, elsewhere) | 2 | no |
//! | Settings, Windows Input Experience (suspended ghosts) | 2 | yes |
//!
//! Cloaking alone does not separate them — a window on another desktop is
//! cloaked too. The pair does.

#![cfg(windows)]

use footman::windows::hidden_by_cloaking;

#[test]
fn a_window_that_is_not_cloaked_is_real() {
    assert!(!hidden_by_cloaking(0, true));
}

/// The measured case that makes the rule necessary: a suspended UWP window sits
/// cloaked on the desktop the user is looking at.
#[test]
fn a_cloaked_window_on_this_desktop_is_a_ghost() {
    assert!(hidden_by_cloaking(2, true));
}

/// The measured case that stops the rule going too far: every window on another
/// virtual desktop is cloaked, and the App Action has to be able to find them
/// (DESIGN.md §4).
#[test]
fn a_cloaked_window_on_another_desktop_is_real() {
    assert!(!hidden_by_cloaking(2, false));
}

/// Cloaked by the application itself rather than by the shell. Nothing observed
/// on the reference machine used this, but it means the same thing.
#[test]
fn application_cloaking_hides_a_window_wherever_it_is() {
    assert!(hidden_by_cloaking(1, true));
    assert!(hidden_by_cloaking(1, false));
}
