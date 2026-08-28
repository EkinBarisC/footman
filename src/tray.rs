//! What the tray shows and what a click means (DESIGN.md §10).
//!
//! Platform-free, like the rest of the Core: the tray is a function of one
//! value, and the Shell only draws the answer and reports clicks.

/// Whether Footman is watching the keyboard.
///
/// Named for what the user cares about rather than for the hook: a paused
/// Footman and a broken one both leave the keyboard entirely normal, and the
/// difference between them is whose decision that was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duty {
    /// The hook is installed and Hyper works.
    Active,
    /// The user asked Footman to stand down. The hook is uninstalled, not
    /// merely ignoring events — for gaming, screen sharing, or handing the
    /// keyboard to someone else (DESIGN.md §10).
    Paused,
    /// Footman could not install the hook (DESIGN.md §7). Never silently dead.
    Broken,
}

/// A menu entry the user chose. Left-clicking the icon is `Settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Settings,
    PauseOrResume,
    Quit,
}

/// What the Shell should do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEffect {
    OpenSettings,
    Pause,
    Resume,
    Quit,
}

impl Duty {
    /// The label of the one entry whose meaning changes with state.
    pub fn pause_label(self) -> &'static str {
        match self {
            Duty::Active => "Pause Footman",
            Duty::Paused => "Resume Footman",
            // The same entry becomes a manual retry. A menu item that does
            // nothing in the one state where the user most wants to act would
            // be worse than no entry at all.
            Duty::Broken => "Try again",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Duty::Active => "Footman — watching for Hyper",
            Duty::Paused => "Footman — paused, your keyboard is untouched",
            Duty::Broken => "Footman — could not watch the keyboard",
        }
    }

    pub fn resolve(self, click: Click) -> TrayEffect {
        match click {
            Click::Settings => TrayEffect::OpenSettings,
            Click::Quit => TrayEffect::Quit,
            Click::PauseOrResume => match self {
                Duty::Active => TrayEffect::Pause,
                Duty::Paused | Duty::Broken => TrayEffect::Resume,
            },
        }
    }

    /// The icon as 32×32 RGBA.
    ///
    /// Drawn in code rather than shipped as a file: three states need three
    /// images, and a mark this simple is less trouble to draw than to keep in
    /// step with itself across three binaries.
    pub fn icon(self) -> Vec<u8> {
        /// The mark, one character per cell, scaled up to fill the icon.
        const MARK: [&str; 8] = [
            ".######.", ".######.", ".##.....", ".#####..", ".#####..", ".##.....", ".##.....",
            "........",
        ];
        const SIDE: usize = 32;
        let scale = SIDE / MARK.len();

        // A paused Footman is drawn dim rather than in another colour: it is
        // the same Footman, standing down. A broken one is red because it is
        // the only state the user has to do something about.
        let ink: [u8; 3] = match self {
            Duty::Active => [232, 232, 232],
            Duty::Paused => [122, 122, 122],
            Duty::Broken => [229, 72, 77],
        };

        let mut rgba = vec![0u8; SIDE * SIDE * 4];
        for y in 0..SIDE {
            for x in 0..SIDE {
                if MARK[y / scale].as_bytes()[x / scale] != b'#' {
                    continue;
                }
                let at = (y * SIDE + x) * 4;
                rgba[at..at + 3].copy_from_slice(&ink);
                rgba[at + 3] = 255;
            }
        }
        rgba
    }
}
