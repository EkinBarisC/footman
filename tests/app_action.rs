//! Which window an App Action raises, and when it launches instead.
//!
//! DESIGN.md §4 — the only Action with real edge cases. The rule is stated over
//! an ordered list of an application's windows, so it is decided in the Core and
//! tested without an operating system. The Shell's job is only to produce that
//! list and to carry out the answer.

use footman::{AppTarget, Window, choose_window};

/// Windows arrive in Z-order, topmost first — the order `EnumWindows` gives.
fn here(id: u64) -> Window {
    Window {
        id,
        on_current_desktop: true,
    }
}

fn elsewhere(id: u64) -> Window {
    Window {
        id,
        on_current_desktop: false,
    }
}

#[test]
fn an_application_that_is_not_running_is_launched() {
    assert_eq!(choose_window(&[], None), AppTarget::Launch);
}

#[test]
fn a_single_window_is_raised() {
    assert_eq!(choose_window(&[here(10)], None), AppTarget::Raise(10));
}

/// "Raise the one nearest the top of the Z-order" — Alt-Tab intuition, and no
/// state to track.
#[test]
fn the_topmost_window_is_raised() {
    assert_eq!(
        choose_window(&[here(10), here(20), here(30)], None),
        AppTarget::Raise(10)
    );
}

/// The current desktop is searched first, so a user who partitions work across
/// desktops usually never leaves the one they are on.
#[test]
fn a_window_on_this_desktop_wins_over_a_higher_one_elsewhere() {
    assert_eq!(
        choose_window(&[elsewhere(10), here(20)], None),
        AppTarget::Raise(20)
    );
}

/// When it genuinely is not here, we go to it — the Shell's raise carries the
/// desktop switch with it.
#[test]
fn an_application_only_on_another_desktop_is_raised_there() {
    assert_eq!(
        choose_window(&[elsewhere(10), elsewhere(20)], None),
        AppTarget::Raise(10)
    );
}

/// Not frontmost, merely visible: raise, do not cycle.
#[test]
fn a_window_that_is_not_frontmost_is_raised_rather_than_cycled() {
    assert_eq!(
        choose_window(&[here(10), here(20)], Some(999)),
        AppTarget::Raise(10)
    );
}

#[test]
fn repeating_the_chord_on_a_lone_window_does_nothing() {
    assert_eq!(
        choose_window(&[here(10), elsewhere(20)], Some(10)),
        AppTarget::Nothing
    );
}

#[test]
fn the_cycle_is_confined_to_the_current_desktop() {
    // Two windows elsewhere are no reason to fling the user off this desktop.
    assert_eq!(
        choose_window(&[here(10), elsewhere(20), elsewhere(30)], Some(10)),
        AppTarget::Nothing
    );
}

/// The point of the cycle: pressing the Chord repeatedly must visit every window
/// of the application on this desktop, not bounce between the last two.
///
/// This is why the cycle runs over a stable order rather than the Z-order. Each
/// raise rewrites the Z-order — the window just raised becomes topmost — so
/// "the next one down" would return to where it came from and never reach the
/// third window.
#[test]
fn cycling_visits_every_window_before_repeating() {
    let mut zorder = vec![here(30), here(10), here(20)];
    let mut foreground = 30;
    let mut visited = vec![foreground];

    for _ in 0..3 {
        let AppTarget::Raise(next) = choose_window(&zorder, Some(foreground)) else {
            panic!("the cycle must keep moving while several windows are here");
        };
        // What raising does to the world: that window is now foreground and
        // topmost.
        zorder.retain(|window| window.id != next);
        zorder.insert(0, here(next));
        foreground = next;
        visited.push(next);
    }

    assert_eq!(
        visited,
        vec![30, 10, 20, 30],
        "the cycle must wrap, not bounce"
    );
}
