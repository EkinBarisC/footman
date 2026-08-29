//! The editable form behind the settings window (DESIGN.md §10).
//!
//! The window is drawn by the Shell, but what it is drawing is a plain value:
//! a list of Bindings the user is part-way through editing, which is not yet a
//! Config and may never become one. Keeping that value in the Core is what
//! makes "two rows want the same Chord" answerable without a screen.

use footman::{Action, Config, Key, Problem, Settings, TapAction};

fn chrome() -> Action {
    Action::App {
        id: "aumid:Chrome".to_string(),
    }
}

fn settings() -> Settings {
    Settings::from(Config::default())
}

#[test]
fn a_config_survives_a_trip_through_the_form() {
    let mut before = Config {
        hyper: Key::F13,
        tap: TapAction::Escape,
        autostart: true,
        ..Config::default()
    };
    before.bindings.add("C".parse().unwrap(), chrome());

    let after = Settings::from(before.clone()).into_config();

    assert_eq!(after.hyper, before.hyper);
    assert_eq!(after.tap, before.tap);
    assert_eq!(after.autostart, before.autostart);
    assert_eq!(after.bindings.sorted(), before.bindings.sorted());
}

/// The form holds rows, not a Binding table, precisely so that a Chord typed
/// twice can be *seen*. A table would have swallowed the second one silently,
/// and the user would be left wondering why their new Binding does nothing.
#[test]
fn two_rows_wanting_the_same_chord_are_reported() {
    let mut settings = settings();
    settings.add("C".parse().unwrap(), chrome());
    settings.add("C".parse().unwrap(), Action::Desktop { index: 2 });

    assert_eq!(
        settings.problems(),
        vec![Problem::DuplicateChord { row: 1 }],
        "the second row is the problem, not the first"
    );
}

#[test]
fn distinct_chords_are_no_problem() {
    let mut settings = settings();
    settings.add("C".parse().unwrap(), chrome());
    settings.add("Shift+C".parse().unwrap(), chrome());

    assert!(settings.problems().is_empty());
}

/// An Action the user has started but not finished. The settings window has to
/// let a half-written row exist — you cannot type a path one character at a
/// time otherwise — so incompleteness is reported rather than prevented.
#[test]
fn an_action_with_nothing_in_it_is_reported() {
    let mut settings = settings();
    settings.add(
        "C".parse().unwrap(),
        Action::Open {
            target: String::new(),
        },
    );

    assert_eq!(settings.problems(), vec![Problem::EmptyAction { row: 0 }]);
}

/// Desktops are numbered from one, as the user counts them. Zero is the
/// mistake `Action::check` already refuses in a config file, and the form has
/// to refuse it before it ever gets written.
#[test]
fn desktop_zero_is_reported() {
    let mut settings = settings();
    settings.add("C".parse().unwrap(), Action::Desktop { index: 0 });

    assert_eq!(settings.problems(), vec![Problem::NoSuchDesktop { row: 0 }]);
}

#[test]
fn a_removed_row_is_gone() {
    let mut settings = settings();
    settings.add("C".parse().unwrap(), chrome());
    settings.add("J".parse().unwrap(), chrome());

    settings.remove(0);

    assert_eq!(settings.bindings.len(), 1);
    assert_eq!(settings.bindings[0].0, "J".parse().unwrap());
}

/// Saving a form with a duplicate must not quietly write both and let the
/// loader pick. The problems are the gate; what gets written is the rest.
#[test]
fn only_the_first_of_a_duplicated_chord_is_written() {
    let mut settings = settings();
    settings.add("C".parse().unwrap(), chrome());
    settings.add("C".parse().unwrap(), Action::Desktop { index: 2 });

    let written = settings.into_config();

    assert_eq!(written.bindings.sorted().len(), 1);
    assert_eq!(*written.bindings.sorted()[0].1, chrome());
}

/// Every row is checked, not just the first one to go wrong: a user with three
/// mistakes should see three, rather than fixing one and being shown the next.
#[test]
fn every_problem_is_reported_at_once() {
    let mut settings = settings();
    settings.add("C".parse().unwrap(), Action::Desktop { index: 0 });
    settings.add(
        "J".parse().unwrap(),
        Action::Run {
            command: String::new(),
            show_window: false,
        },
    );
    settings.add("J".parse().unwrap(), chrome());

    assert_eq!(
        settings.problems(),
        vec![
            Problem::NoSuchDesktop { row: 0 },
            Problem::EmptyAction { row: 1 },
            Problem::DuplicateChord { row: 2 },
        ]
    );
}

// A status line is a thing said, not a thing set. Saying it and never taking it
// back leaves "Saved" sitting under a form the user has since edited — and,
// because the window is hidden rather than closed, still sitting there the next
// time they open it. So a Notice knows when it was said and stops being true.

use footman::Notice;

#[test]
fn a_notice_is_there_when_it_is_said() {
    let mut notice = Notice::default();
    notice.say("Saved", 10.0);

    assert_eq!(notice.text(10.0), "Saved");
}

#[test]
fn a_notice_fades() {
    let mut notice = Notice::default();
    notice.say("Saved", 10.0);

    assert_eq!(notice.text(12.0), "Saved", "still worth reading");
    assert_eq!(notice.text(30.0), "", "long since read");
}

/// What closing the settings window does: whatever was said is over.
#[test]
fn a_notice_can_be_taken_back_at_once() {
    let mut notice = Notice::default();
    notice.say("Saved", 10.0);
    notice.clear();

    assert_eq!(notice.text(10.0), "");
}

#[test]
fn nothing_said_is_nothing_shown() {
    assert_eq!(Notice::default().text(0.0), "");
}
