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

/// What the shell's list of installed applications hands back, and why it
/// cannot be used as an identity unaltered.
///
/// A packaged application's entry *is* its AUMID. A classic one's is a path
/// relative to a known folder — `{6D809377-…}\7-Zip\7zFM.exe` — which the shell
/// can launch but which matches no window: a running 7-Zip resolves to its
/// executable path, not to that. Binding it verbatim would give a Chord that
/// opens a second copy every time instead of raising the first.
mod installed_entries {
    use footman::windows::{Entry, entry_of};

    #[test]
    fn a_packaged_application_is_already_an_aumid() {
        assert_eq!(
            entry_of("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"),
            Entry::Aumid("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".to_string())
        );
        assert_eq!(entry_of("Chrome"), Entry::Aumid("Chrome".to_string()));
    }

    #[test]
    fn a_classic_application_is_a_path_under_a_known_folder() {
        assert_eq!(
            entry_of(r"{6D809377-6AF0-444B-8957-A3773F02200E}\7-Zip\7zFM.exe"),
            Entry::UnderFolder {
                folder: "{6D809377-6AF0-444B-8957-A3773F02200E}".to_string(),
                relative: r"7-Zip\7zFM.exe".to_string(),
            }
        );
    }

    /// Anything that only looks like one. Treated as an AUMID, which is the
    /// reading that still launches.
    #[test]
    fn a_name_that_is_not_shaped_like_one_is_left_alone() {
        assert_eq!(
            entry_of("{not-a-folder"),
            Entry::Aumid("{not-a-folder".to_string())
        );
        assert_eq!(
            entry_of(r"{6D809377-6AF0-444B-8957-A3773F02200E}"),
            Entry::Aumid(r"{6D809377-6AF0-444B-8957-A3773F02200E}".to_string())
        );
    }
}
