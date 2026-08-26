//! Reading, comparing and acting on App Identities (ADR-0003).
//!
//! An App Identity is an opaque, scheme-prefixed string. The Core stores and
//! compares it without understanding it; this module is the only place that
//! knows `aumid:` from `path:`.

/// How to start an application, once its identity has been read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// Through `shell:AppsFolder`, the shell's virtual folder of every
    /// installed application. It starts MSIX and classic applications alike,
    /// and is the only documented way to start one by AUMID.
    AppsFolder(String),
    /// A plain executable path.
    Executable(String),
}

/// Reads an identity as something startable, or `None` if this build does not
/// know the scheme — a hand-edited config, or one written by a later version.
/// Guessing would launch the wrong thing.
pub fn launch_of(identity: &str) -> Option<Launch> {
    if let Some(aumid) = identity.strip_prefix("aumid:") {
        return (!aumid.is_empty()).then(|| Launch::AppsFolder(aumid.to_string()));
    }
    if let Some(path) = identity.strip_prefix("path:") {
        return (!path.is_empty()).then(|| Launch::Executable(path.to_string()));
    }
    None
}

/// Whether a window whose cascade produced `cascade` is the application named
/// by `identity`.
///
/// A window matches on *any* step of its cascade rather than only the first.
/// The order in ADR-0003 decides which identity gets *stored*; for matching, a
/// window whose window-AUMID differs may still be the same application by its
/// process AUMID or its executable path. Comparison is case-insensitive:
/// Windows paths are, and so are AUMIDs.
pub fn matches(identity: &str, cascade: &[String]) -> bool {
    cascade
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(identity))
}
