//! The editable form behind the settings window (DESIGN.md §10).
//!
//! A Config is the settled thing on disk. This is the unsettled thing on
//! screen: a list of rows the user is part-way through editing, which may hold
//! two rows wanting the same Chord, or an Action with nothing typed into it
//! yet. Both states have to be allowed to exist — you cannot type a path one
//! character at a time otherwise — so they are *reported* rather than
//! prevented, and this is where that reporting lives.

use crate::{Action, BindingTable, Chord, Config, Key, TapAction};

/// Something wrong with a row, and which row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// An earlier row already claims this Chord. The earlier one wins, exactly
    /// as it does when a config file is loaded (DESIGN.md §7).
    DuplicateChord { row: usize },
    /// An Action whose target, command or identity is still empty.
    EmptyAction { row: usize },
    /// Desktops are numbered from one, as the user counts them.
    NoSuchDesktop { row: usize },
}

impl Problem {
    pub fn row(self) -> usize {
        match self {
            Problem::DuplicateChord { row }
            | Problem::EmptyAction { row }
            | Problem::NoSuchDesktop { row } => row,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Problem::DuplicateChord { .. } => "another Binding already uses this Chord",
            Problem::EmptyAction { .. } => "this Action has nothing to do",
            Problem::NoSuchDesktop { .. } => "desktops are numbered from 1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub hyper: Key,
    pub tap: TapAction,
    pub autostart: bool,
    /// Rows rather than a Binding table, so a Chord typed twice can be seen.
    /// A table would swallow the second one and leave the user wondering why
    /// their new Binding does nothing.
    pub bindings: Vec<(Chord, Action)>,
}

impl From<Config> for Settings {
    fn from(config: Config) -> Self {
        Settings {
            hyper: config.hyper,
            tap: config.tap,
            autostart: config.autostart,
            bindings: config
                .bindings
                .sorted()
                .into_iter()
                .map(|(chord, action)| (chord, action.clone()))
                .collect(),
        }
    }
}

impl Settings {
    pub fn add(&mut self, chord: Chord, action: Action) {
        self.bindings.push((chord, action));
    }

    pub fn remove(&mut self, row: usize) {
        if row < self.bindings.len() {
            self.bindings.remove(row);
        }
    }

    /// Everything wrong with the form, in row order.
    ///
    /// All of it at once: a user with three mistakes should see three, rather
    /// than fixing one and being shown the next.
    pub fn problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        let mut claimed = Vec::new();

        for (row, (chord, action)) in self.bindings.iter().enumerate() {
            if claimed.contains(chord) {
                problems.push(Problem::DuplicateChord { row });
            } else {
                claimed.push(*chord);
            }

            match action {
                Action::App { id } if id.is_empty() => problems.push(Problem::EmptyAction { row }),
                Action::Open { target } if target.is_empty() => {
                    problems.push(Problem::EmptyAction { row })
                }
                Action::Run { command, .. } if command.trim().is_empty() => {
                    problems.push(Problem::EmptyAction { row })
                }
                Action::Desktop { index: 0 } => problems.push(Problem::NoSuchDesktop { row }),
                _ => {}
            }
        }

        problems
    }

    /// The form as something that can be written to disk.
    ///
    /// Duplicates collapse the same way the loader collapses them — first wins
    /// — rather than writing both and letting the next load decide. What stops
    /// a duplicate reaching this point at all is `problems`.
    pub fn into_config(self) -> Config {
        let mut bindings = BindingTable::empty();
        for (chord, action) in self.bindings {
            bindings.add(chord, action);
        }

        Config {
            hyper: self.hyper,
            tap: self.tap,
            autostart: self.autostart,
            bindings,
        }
    }
}

/// Something the settings window is saying, and when it said it.
///
/// A status line that is only ever set goes stale: "Saved" stays under a form
/// the user has since edited, and — because the window is hidden rather than
/// closed (DESIGN.md §10) — it is still there the next time they open it. So a
/// Notice carries the moment it was said and stops being true after a while.
///
/// The clock is passed in rather than read, which is what lets this be tested
/// at all: the Shell has one in `egui`'s frame time.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Notice {
    said: Option<(String, f64)>,
}

impl Notice {
    /// How long anything is worth reading. Long enough to notice, short enough
    /// that it is gone before it can mislead.
    const LIFETIME: f64 = 6.0;

    pub fn say(&mut self, text: impl Into<String>, now: f64) {
        self.said = Some((text.into(), now));
    }

    pub fn clear(&mut self) {
        self.said = None;
    }

    pub fn text(&self, now: f64) -> &str {
        match &self.said {
            Some((text, said)) if now - said < Self::LIFETIME => text,
            _ => "",
        }
    }
}
