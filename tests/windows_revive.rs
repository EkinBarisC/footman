//! When to rebuild the tray and settings window after they die (ADR-0008).
//!
//! The window can be lost to things Footman does not control — a GL context
//! gone after the laptop resumes from hibernation is the one that prompted
//! this. The hook never depended on it, so the window is rebuilt rather than
//! the whole of Footman exiting; but a window that cannot exist on this machine
//! must not be rebuilt in a loop nobody can see.

#![cfg(windows)]

use footman::windows::{Recovery, Revival};

const LIMIT: usize = 5;
const SPAN: u64 = 10 * 60 * 1_000;

#[test]
fn a_window_lost_once_is_rebuilt() {
    let mut revival = Revival::new(LIMIT, SPAN);

    assert!(matches!(revival.died(1_000), Recovery::After(_)));
}

/// Each rebuild in quick succession waits longer than the last, giving
/// whatever is wrong underneath more time to settle.
#[test]
fn a_window_that_keeps_dying_is_given_longer_each_time() {
    let mut revival = Revival::new(LIMIT, SPAN);

    let Recovery::After(first) = revival.died(1_000) else {
        panic!("the first death is rebuilt");
    };
    let Recovery::After(second) = revival.died(2_000) else {
        panic!("the second death is rebuilt");
    };

    assert!(second > first);
}

#[test]
fn no_wait_is_unreasonably_long() {
    let mut revival = Revival::new(100, SPAN);

    for death in 0..40 {
        if let Recovery::After(wait) = revival.died(death) {
            assert!(wait <= 60_000, "death {death} waits {wait} ms");
        }
    }
}

/// Too many deaths too close together means the window cannot exist here.
#[test]
fn a_window_that_dies_as_fast_as_it_is_built_is_abandoned() {
    let mut revival = Revival::new(LIMIT, SPAN);

    for death in 0..LIMIT as u64 {
        assert!(matches!(revival.died(death * 1_000), Recovery::After(_)));
    }

    assert_eq!(revival.died(LIMIT as u64 * 1_000), Recovery::GiveUp);
}

/// A laptop that sleeps every night loses its window every night. That is a
/// window to rebuild every night, not a budget to exhaust by the end of the
/// week.
#[test]
fn deaths_far_apart_never_add_up() {
    let mut revival = Revival::new(LIMIT, SPAN);
    let night = 24 * 60 * 60 * 1_000;

    for day in 0..30 {
        assert!(matches!(revival.died(day * night), Recovery::After(_)));
    }
}

/// Once the span has passed, the count starts again — and so does the wait.
#[test]
fn an_old_death_is_forgotten() {
    let mut revival = Revival::new(LIMIT, SPAN);
    let Recovery::After(fresh) = revival.died(0) else {
        panic!("the first death is rebuilt");
    };

    assert_eq!(revival.died(SPAN + 1), Recovery::After(fresh));
}
