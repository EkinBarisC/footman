//! Reading virtual desktop state, and working out how to get to one (ADR-0004).
//!
//! Windows exposes no documented way to switch desktops, so Footman reads where
//! it is from the registry and walks there with synthetic arrow presses. Both
//! halves are arithmetic over bytes and indices, and are tested here without a
//! registry or a keyboard: what the Shell contributes is only fetching the two
//! blobs and emitting the presses.

#![cfg(windows)]

use footman::windows::{Desktops, Move, desktops_from, steps_to};

/// `VirtualDesktopIDs` is an ordered array of 16-byte GUIDs, back to back, and
/// `CurrentVirtualDesktop` is one of them.
fn guid(tag: u8) -> [u8; 16] {
    [tag; 16]
}

fn ids(tags: &[u8]) -> Vec<u8> {
    tags.iter().flat_map(|&tag| guid(tag)).collect()
}

#[test]
fn the_first_desktop_is_index_one() {
    assert_eq!(
        desktops_from(&ids(&[1, 2, 3]), &guid(1)),
        Some(Desktops {
            current: 1,
            count: 3
        })
    );
}

/// Indices are the ones a user writes in a config — `desktop = 2` is the second
/// desktop, not the third.
#[test]
fn position_in_the_array_is_the_desktop_index() {
    assert_eq!(
        desktops_from(&ids(&[1, 2, 3]), &guid(3)),
        Some(Desktops {
            current: 3,
            count: 3
        })
    );
}

#[test]
fn a_single_desktop_is_read_as_one_desktop() {
    assert_eq!(
        desktops_from(&ids(&[7]), &guid(7)),
        Some(Desktops {
            current: 1,
            count: 1
        })
    );
}

/// The values are undocumented, so Footman must not assume their shape. Every
/// malformed reading means "do not know", never a guess: guessing here moves
/// the user somewhere they did not ask to be.
#[test]
fn a_reading_that_does_not_make_sense_is_refused() {
    // A list that is not a whole number of GUIDs.
    assert_eq!(desktops_from(&[0u8; 20], &guid(1)), None);
    // No desktops at all.
    assert_eq!(desktops_from(&[], &guid(1)), None);
    // A current desktop that is not one of the listed ones.
    assert_eq!(desktops_from(&ids(&[1, 2]), &guid(9)), None);
    // A current value that is not a GUID.
    assert_eq!(desktops_from(&ids(&[1, 2]), &[1u8; 8]), None);
}

fn at(current: usize, count: usize) -> Desktops {
    Desktops { current, count }
}

#[test]
fn moving_right_takes_one_press_per_desktop() {
    assert_eq!(steps_to(at(1, 4), 3), Some(Move::Right(2)));
}

#[test]
fn moving_left_takes_one_press_per_desktop() {
    assert_eq!(steps_to(at(4, 4), 1), Some(Move::Left(3)));
}

/// Asking for the desktop you are already on is not an error, and must not send
/// a single keystroke — an arrow press that cancels itself out would still be
/// visible as an animation.
#[test]
fn asking_for_the_current_desktop_sends_nothing() {
    assert_eq!(steps_to(at(2, 4), 2), Some(Move::Stay));
}

/// A desktop that does not exist is refused rather than clamped. Walking to the
/// nearest one instead would silently do something the user did not ask for,
/// and they would never learn their config was wrong.
#[test]
fn a_desktop_that_does_not_exist_is_refused() {
    assert_eq!(steps_to(at(2, 3), 4), None);
    assert_eq!(steps_to(at(2, 3), 0), None);
}

/// Crossing two desktops is two arrow presses whatever is done about the
/// animation between them: what `switch_to` suppresses is the drawing, not the
/// journey.
#[test]
fn suppressing_the_animation_does_not_change_how_far_we_go() {
    assert_eq!(steps_to(at(1, 4), 4), Some(Move::Right(3)));
    assert_eq!(steps_to(at(3, 4), 1), Some(Move::Left(2)));
}
