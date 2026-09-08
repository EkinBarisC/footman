//! What the tray shows and what a click means (DESIGN.md §10).
//!
//! The tray is the only surface Footman has until the settings window, and it
//! is where the user learns whether their keyboard is being watched at all. All
//! of that is a function of one value, so it is decided in the Core and tested
//! without an operating system; the Shell only draws it and reports clicks.

use footman::{Click, Duty, TrayEffect};

#[test]
fn the_menu_offers_to_pause_while_footman_is_working() {
    assert_eq!(Duty::Active.pause_label(), "Pause Footman");
    assert_eq!(
        Duty::Active.resolve(Click::PauseOrResume),
        TrayEffect::Pause
    );
}

#[test]
fn the_menu_offers_to_resume_once_it_is_paused() {
    assert_eq!(Duty::Paused.pause_label(), "Resume Footman");
    assert_eq!(
        Duty::Paused.resolve(Click::PauseOrResume),
        TrayEffect::Resume
    );
}

/// The hook could not be installed (DESIGN.md §7). The same entry becomes a
/// manual retry: the alternative is a menu item that does nothing in the one
/// state where the user most wants to act.
#[test]
fn a_broken_footman_offers_to_try_again() {
    assert_eq!(Duty::Broken.pause_label(), "Try again");
    assert_eq!(
        Duty::Broken.resolve(Click::PauseOrResume),
        TrayEffect::Resume
    );
}

#[test]
fn settings_and_quit_mean_the_same_thing_in_every_state() {
    for duty in [Duty::Active, Duty::Paused, Duty::Broken] {
        assert_eq!(duty.resolve(Click::Settings), TrayEffect::OpenSettings);
        assert_eq!(duty.resolve(Click::Quit), TrayEffect::Quit);
    }
}

/// The tooltip is the whole of Footman's status reporting until slice 7, so a
/// paused Footman must never look like a working one.
#[test]
fn every_state_says_something_different() {
    let tooltips: Vec<&str> = [Duty::Active, Duty::Paused, Duty::Broken]
        .iter()
        .map(|duty| duty.tooltip())
        .collect();

    assert_eq!(tooltips.len(), 3);
    for (index, tooltip) in tooltips.iter().enumerate() {
        assert!(!tooltip.is_empty());
        assert!(
            !tooltips[..index].contains(tooltip),
            "two states share the tooltip {tooltip:?}"
        );
    }
}

/// A user glancing at the tray reads the icon, not the tooltip. Three states
/// that render the same pixels would be three states they cannot tell apart.
#[test]
fn every_state_looks_different() {
    let icons: Vec<Vec<u8>> = [Duty::Active, Duty::Paused, Duty::Broken]
        .iter()
        .map(|duty| duty.icon())
        .collect();

    for icon in &icons {
        assert_eq!(icon.len(), 32 * 32 * 4, "icons are 32×32 RGBA");
        assert!(
            icon.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 255),
            "an icon with nothing opaque in it is an invisible icon"
        );
    }

    assert_ne!(icons[0], icons[1]);
    assert_ne!(icons[1], icons[2]);
    assert_ne!(icons[0], icons[2]);
}
