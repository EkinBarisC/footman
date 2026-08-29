//! The list of installed applications the settings window offers (DESIGN.md §10).
//!
//! Not a palette. It exists only while a Binding is being created, and never
//! appears on the hot path — Footman has no search surface at all, which is
//! most of why it exists.
//!
//! The list comes from `AppsFolder`, the shell's own virtual folder of every
//! installed application. That is deliberately the same place `launch` starts
//! things from (ADR-0003), so an identity picked here is one Footman can
//! certainly start.

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, KF_FLAG_DEFAULT,
    SHGetKnownFolderItem, SHGetKnownFolderPath, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::GUID;

/// How the shell names an entry in its list of installed applications.
///
/// The two shapes want opposite treatment. A packaged application's parsing
/// name *is* its AUMID and can be bound as it stands. A classic application's
/// is a path relative to one of Windows' known folders, which the shell can
/// launch but which matches no window — a running copy resolves to its
/// executable path, not to that. Binding it verbatim would give a Chord that
/// opened a second copy every time instead of raising the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Aumid(String),
    UnderFolder { folder: String, relative: String },
}

/// Reads the shape. Anything that only looks like a folder-relative path is
/// treated as an AUMID, which is the reading that still launches.
pub fn entry_of(parsing_name: &str) -> Entry {
    let folder_and_rest = parsing_name
        .strip_prefix('{')
        .and_then(|rest| rest.split_once("}\\"));

    match folder_and_rest {
        Some((folder, relative)) if !relative.is_empty() => Entry::UnderFolder {
            folder: format!("{{{folder}}}"),
            relative: relative.to_string(),
        },
        _ => Entry::Aumid(parsing_name.to_string()),
    }
}

/// One entry: what the user sees, and what gets written into the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub name: String,
    /// An App Identity, already scheme-prefixed.
    pub identity: String,
}

/// Every installed application, sorted by name.
///
/// Returns an empty list rather than an error if the shell will not answer:
/// the settings window still has the escape hatch of picking an executable
/// directly, so a failure here costs the convenience and not the feature.
pub fn applications() -> Vec<Installed> {
    let mut found = Vec::new();

    unsafe {
        let Ok(folder) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
        else {
            return found;
        };
        let Ok(items) = folder
            .BindToHandler::<Option<&windows::Win32::System::Com::IBindCtx>, IEnumShellItems>(
                None,
                &BHID_EnumItems,
            )
        else {
            return found;
        };

        loop {
            let mut batch = [const { None }; 1];
            let mut fetched = 0u32;
            if items.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = batch[0].take() else { break };

            // For a child of AppsFolder the parent-relative parsing name is
            // what `shell:AppsFolder\…` takes — an AUMID for a packaged
            // application, a known-folder-relative path for a classic one.
            let (Some(name), Some(parsing_name)) = (
                display_name(&item, SIGDN_NORMALDISPLAY),
                display_name(&item, SIGDN_PARENTRELATIVEPARSING),
            ) else {
                continue;
            };

            found.push(Installed {
                name,
                identity: identity_of(&parsing_name),
            });
        }
    }

    found.sort_by_key(|app| app.name.to_lowercase());
    found.dedup_by(|a, b| a.identity == b.identity);
    found
}

/// The identity to write into a config for an entry the shell named this way.
///
/// A classic application becomes a `path:` identity rather than the shell's own
/// spelling, because that is what a window of it will answer to (ADR-0003). If
/// the known folder cannot be resolved, the shell's spelling is kept: it will
/// still launch, which is better than nothing to bind at all.
fn identity_of(parsing_name: &str) -> String {
    match entry_of(parsing_name) {
        Entry::Aumid(aumid) => format!("aumid:{aumid}"),
        Entry::UnderFolder { folder, relative } => match known_folder(&folder) {
            Some(base) => format!(r"path:{base}\{relative}"),
            None => format!("aumid:{parsing_name}"),
        },
    }
}

fn known_folder(folder: &str) -> Option<String> {
    // The shell writes the braces; `GUID` will not read them.
    let id = GUID::try_from(folder.trim_matches(['{', '}'])).ok()?;
    unsafe {
        let text = SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None).ok()?;
        let owned = text.to_string().ok()?;
        CoTaskMemFree(Some(text.0.cast()));
        Some(owned)
    }
}

fn display_name(item: &IShellItem, kind: windows::Win32::UI::Shell::SIGDN) -> Option<String> {
    unsafe {
        let text = item.GetDisplayName(kind).ok()?;
        let owned = text.to_string().ok()?;
        CoTaskMemFree(Some(text.0.cast()));
        (!owned.is_empty()).then_some(owned)
    }
}
