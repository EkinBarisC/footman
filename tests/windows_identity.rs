//! Reading and comparing App Identities (ADR-0003).
//!
//! The identity itself is opaque to the Core; the Shell is the only half that
//! knows `aumid:` from `path:`. That parsing is ordinary code and is tested
//! here without touching a window.

#![cfg(windows)]

use footman::windows::{Launch, launch_of, matches};

#[test]
fn an_aumid_identity_launches_through_the_apps_folder() {
    // The documented way to start an application by AUMID: the shell's own
    // virtual folder, which handles MSIX and classic applications alike.
    assert_eq!(
        launch_of("aumid:Chrome"),
        Some(Launch::AppsFolder("Chrome".to_string()))
    );
}

#[test]
fn a_path_identity_launches_the_executable() {
    assert_eq!(
        launch_of(r"path:C:\Program Files\Unity Hub\Unity Hub.exe"),
        Some(Launch::Executable(
            r"C:\Program Files\Unity Hub\Unity Hub.exe".to_string()
        ))
    );
}

/// An identity Footman did not write — a hand-edited config, or one from a
/// future version with a scheme this build does not know.
#[test]
fn an_unknown_scheme_is_not_guessed_at() {
    assert_eq!(launch_of("bundle:com.apple.Safari"), None);
    assert_eq!(launch_of(r"C:\chrome.exe"), None);
    assert_eq!(launch_of(""), None);
}

/// A window matches if *any* step of its cascade produces the identity, not
/// only the first. The three steps are tried in order for storing an identity,
/// but a window whose window-AUMID differs may still be the same application by
/// its process AUMID or its path.
#[test]
fn a_window_matches_on_any_step_of_its_cascade() {
    let cascade = [
        "aumid:Chrome".to_string(),
        r"path:C:\chrome.exe".to_string(),
    ];

    assert!(matches("aumid:Chrome", &cascade));
    assert!(matches(r"path:C:\chrome.exe", &cascade));
    assert!(!matches("aumid:Firefox", &cascade));
}

/// Windows paths are case-insensitive, and so are AUMIDs. A config that says
/// `path:c:\chrome.exe` must not silently stop matching.
#[test]
fn identities_compare_case_insensitively() {
    let cascade = [r"path:C:\Program Files\Chrome.exe".to_string()];

    assert!(matches(r"path:c:\program files\chrome.exe", &cascade));
}

#[test]
fn a_window_with_no_identity_at_all_matches_nothing() {
    assert!(!matches("aumid:Chrome", &[]));
}
