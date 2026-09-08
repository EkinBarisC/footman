//! Behaviour of config loading.
//!
//! The config file is the single source of truth (DESIGN.md §6): the settings
//! window writes it and nothing else holds the state. It is therefore also
//! hand-editable, which is why the failure behaviour in DESIGN.md §7 matters as
//! much as the happy path.

use footman::{Action, Chord, Config, Key, TapAction, Warning};

#[test]
fn a_minimal_config_yields_its_binding() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "C"
        action = { type = "app", id = "aumid:Chrome" }
        "#,
    )
    .expect("valid config");

    assert_eq!(loaded.config.hyper, Key::CapsLock);
    assert_eq!(loaded.config.tap, TapAction::None);
    assert_eq!(
        loaded.config.bindings.get(&Chord::key(Key::C)),
        Some(&Action::App {
            id: "aumid:Chrome".to_string()
        })
    );
    assert!(loaded.warnings.is_empty());
}

#[test]
fn malformed_toml_is_fatal_and_says_which_line() {
    let error = Config::parse(
        r#"
        [[binding]]
        chord  = "C"
        action = { type = "app"
        "#,
    )
    .expect_err("structurally broken file must not load");

    assert_eq!(error.line, Some(4), "the settings window shows this line");
}

#[test]
fn an_empty_file_loads_the_defaults() {
    let loaded = Config::parse("").expect("an empty file is valid");

    assert_eq!(loaded.config.hyper, Key::CapsLock);
    assert_eq!(loaded.config.tap, TapAction::None);
    assert_eq!(loaded.config.bindings.get(&Chord::key(Key::C)), None);
}

#[test]
fn a_binding_with_an_unusable_action_is_skipped_not_fatal() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "C"
        action = { type = "app", id = "aumid:Chrome" }

        [[binding]]
        chord  = "J"
        action = { type = "teleport" }
        "#,
    )
    .expect("one unusable Binding must not sink the whole file");

    assert!(loaded.config.bindings.get(&Chord::key(Key::C)).is_some());
    assert!(loaded.config.bindings.get(&Chord::key(Key::J)).is_none());
    assert!(
        matches!(
            loaded.warnings.as_slice(),
            [Warning::UnusableAction { chord, .. }] if chord == "J"
        ),
        "expected one warning naming the skipped Chord, got {:?}",
        loaded.warnings
    );
}

#[test]
fn a_binding_with_an_unreadable_chord_is_skipped() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "Hyperspace"
        action = { type = "app", id = "aumid:Chrome" }
        "#,
    )
    .expect("an unreadable Chord must not sink the whole file");

    assert!(
        matches!(
            loaded.warnings.as_slice(),
            [Warning::UnreadableChord { chord }] if chord == "Hyperspace"
        ),
        "got {:?}",
        loaded.warnings
    );
}

#[test]
fn a_duplicate_chord_keeps_the_first_and_warns() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "C"
        action = { type = "app", id = "aumid:Chrome" }

        [[binding]]
        chord  = "C"
        action = { type = "app", id = "aumid:Claude_pzs8sxrjxfjjc!Claude" }
        "#,
    )
    .expect("a duplicate is a warning, not a failure");

    assert_eq!(
        loaded.config.bindings.get(&Chord::key(Key::C)),
        Some(&Action::App {
            id: "aumid:Chrome".to_string()
        }),
        "the first Binding wins; the second must never silently overwrite it"
    );
    assert!(
        matches!(
            loaded.warnings.as_slice(),
            [Warning::DuplicateChord { chord }] if chord == "C"
        ),
        "got {:?}",
        loaded.warnings
    );
}

#[test]
fn the_hyper_section_chooses_the_key_and_the_tap_action() {
    let loaded = Config::parse(
        r#"
        [hyper]
        key = "capslock"
        tap = "escape"

        [general]
        autostart = true
        "#,
    )
    .expect("valid config");

    assert_eq!(loaded.config.hyper, Key::CapsLock);
    assert_eq!(loaded.config.tap, TapAction::Escape);
    assert!(loaded.config.autostart);
    assert!(loaded.warnings.is_empty());
}

/// Unlike a single bad Binding, an unreadable Hyper Key is not a localised
/// problem: nothing at all would work. Falling back to Caps Lock would also
/// mean seizing a key the user did not ask for, so this is fatal — Footman
/// installs no hook and the keyboard stays entirely normal (DESIGN.md §7).
#[test]
fn an_unreadable_hyper_key_is_fatal() {
    let error = Config::parse(
        r#"
        [hyper]
        key = "Hyperspace"
        "#,
    )
    .expect_err("Footman cannot run without a Hyper Key");

    assert_eq!(error.line, Some(3));
}

/// A Tap Action is different: without one, Footman simply does nothing on tap,
/// which is also the default. So this is a warning, not a failure.
#[test]
fn an_unreadable_tap_action_falls_back_to_none_with_a_warning() {
    let loaded = Config::parse(
        r#"
        [hyper]
        tap = "somersault"
        "#,
    )
    .expect("a bad Tap Action must not stop Footman running");

    assert_eq!(loaded.config.tap, TapAction::None);
    assert!(
        matches!(
            loaded.warnings.as_slice(),
            [Warning::UnreadableTapAction { .. }]
        ),
        "got {:?}",
        loaded.warnings
    );
}

#[test]
fn letters_digits_function_keys_and_named_keys_all_parse() {
    let cases = [
        ("A", Key::A),
        ("z", Key::Z),
        ("0", Key::Digit0),
        ("9", Key::Digit9),
        ("F1", Key::F1),
        ("f24", Key::F24),
        ("Left", Key::Left),
        ("Space", Key::Space),
        ("Enter", Key::Enter),
        ("Escape", Key::Escape),
        ("CapsLock", Key::CapsLock),
    ];

    for (name, expected) in cases {
        assert_eq!(name.parse::<Key>(), Ok(expected), "parsing {name:?}");
    }
}

/// Punctuation is deliberately unbindable. Where `;` or `[` physically sits
/// depends on the keyboard layout — the same trap that shaped ADR-0001 — so a
/// config naming them would mean different keys on different machines.
#[test]
fn punctuation_keys_are_not_bindable() {
    for name in ["Semicolon", ";", "BracketLeft", "Comma"] {
        assert!(name.parse::<Key>().is_err(), "{name:?} must not parse");
    }
}

#[test]
fn a_key_name_round_trips_through_display() {
    for name in ["A", "0", "F24", "Left", "CapsLock", "Space"] {
        let key: Key = name.parse().expect(name);
        assert_eq!(key.to_string().parse::<Key>(), Ok(key));
    }
}

#[test]
fn a_chord_reads_its_modifiers_in_any_order_or_case() {
    let expected =
        Chord::key(Key::C).with(footman::Modifiers::SHIFT.union(footman::Modifiers::CTRL));

    for text in ["Shift+Ctrl+C", "ctrl+shift+c", " Ctrl + Shift + C "] {
        assert_eq!(text.parse::<Chord>(), Ok(expected), "parsing {text:?}");
    }
}

#[test]
fn all_four_action_kinds_parse() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "C"
        action = { type = "app", id = "aumid:Chrome" }

        [[binding]]
        chord  = "G"
        action = { type = "open", target = "https://github.com" }

        [[binding]]
        chord  = "P"
        action = { type = "run", command = "pnpm dev", show_window = true }

        [[binding]]
        chord  = "1"
        action = { type = "desktop", index = 1 }
        "#,
    )
    .expect("valid config");

    let bindings = &loaded.config.bindings;
    assert_eq!(
        bindings.get(&Chord::key(Key::G)),
        Some(&Action::Open {
            target: "https://github.com".to_string()
        })
    );
    assert_eq!(
        bindings.get(&Chord::key(Key::P)),
        Some(&Action::Run {
            command: "pnpm dev".to_string(),
            show_window: true,
        })
    );
    assert_eq!(
        bindings.get(&Chord::key(Key::Digit1)),
        Some(&Action::Desktop { index: 1 })
    );
    assert!(loaded.warnings.is_empty());
}

/// A console window is noise for a launcher, so `run` hides it unless asked.
#[test]
fn run_hides_its_console_by_default() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "P"
        action = { type = "run", command = "pnpm dev" }
        "#,
    )
    .expect("valid config");

    assert_eq!(
        loaded.config.bindings.get(&Chord::key(Key::P)),
        Some(&Action::Run {
            command: "pnpm dev".to_string(),
            show_window: false,
        })
    );
}

/// Desktops are numbered as the user sees them, from 1. Zero is a mistake, and
/// silently treating it as desktop 1 would hide the mistake.
#[test]
fn desktop_zero_is_rejected_as_a_binding() {
    let loaded = Config::parse(
        r#"
        [[binding]]
        chord  = "1"
        action = { type = "desktop", index = 0 }
        "#,
    )
    .expect("a bad index is a skipped Binding, not a broken file");

    assert!(
        loaded
            .config
            .bindings
            .get(&Chord::key(Key::Digit1))
            .is_none()
    );
    assert!(
        matches!(
            loaded.warnings.as_slice(),
            [Warning::UnusableAction { chord, .. }] if chord == "1"
        ),
        "got {:?}",
        loaded.warnings
    );
}

const FULL: &str = r#"
[hyper]
key = "capslock"
tap = "escape"

[general]
autostart = true

[[binding]]
chord  = "C"
action = { type = "app", id = "aumid:Chrome" }

[[binding]]
chord  = "Ctrl+Shift+G"
action = { type = "open", target = "https://github.com" }

[[binding]]
chord  = "P"
action = { type = "run", command = "pnpm dev" }

[[binding]]
chord  = "1"
action = { type = "desktop", index = 1 }
"#;

#[test]
fn a_config_round_trips_through_the_file_it_writes() {
    let original = Config::parse(FULL).expect("valid config").config;

    let written = original.to_toml();
    let reloaded = Config::parse(&written).expect("what we write, we must be able to read");

    assert_eq!(reloaded.config, original);
    assert!(reloaded.warnings.is_empty(), "got {:?}", reloaded.warnings);
}

/// The settings window rewrites this file on every change, and the user is
/// expected to hand-edit and version-control it. A map's iteration order would
/// reshuffle the Bindings on each save and turn every diff into noise.
#[test]
fn the_written_file_has_a_stable_order() {
    let once = Config::parse(FULL).unwrap().config.to_toml();
    let twice = Config::parse(FULL).unwrap().config.to_toml();

    assert_eq!(once, twice);
}

#[test]
fn a_missing_file_is_created_with_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let loaded = Config::load_or_create(&path).expect("first run must not fail");

    assert_eq!(loaded.config, Config::default());
    assert!(
        path.exists(),
        "the default config is written out, not just returned"
    );
}

#[test]
fn a_saved_config_is_the_one_that_loads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = Config::parse(FULL).unwrap().config;

    original.save(&path).expect("save");
    let loaded = Config::load_or_create(&path).expect("load");

    assert_eq!(loaded.config, original);
    assert!(loaded.warnings.is_empty());
}

#[test]
fn the_default_path_sits_under_the_users_config_directory() {
    let path = Config::default_path().expect("a config directory must exist");

    assert!(
        path.ends_with("footman/config.toml"),
        "got {}",
        path.display()
    );
}
