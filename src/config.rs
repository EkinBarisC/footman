//! Loading the config file.
//!
//! The file is the single source of truth (DESIGN.md §6). It is written by the
//! settings window but is equally meant to be hand-edited, so loading has to
//! distinguish two kinds of failure (DESIGN.md §7): a file whose *structure*
//! cannot be trusted, which is fatal, and a single unusable Binding, which is
//! skipped and reported.

use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::{Action, BindingTable, Chord, Key, TapAction};

/// Everything the running Footman needs, as read from the file.
#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub hyper: Key,
    pub tap: TapAction,
    pub autostart: bool,
    pub bindings: BindingTable,
}

/// A config that loaded, together with anything about it worth reporting.
///
/// Warnings are not errors: the configuration is usable. They exist so the
/// settings window can flag the offending rows rather than silently dropping
/// them.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    pub warnings: Vec<Warning>,
}

/// A Binding that could not be used, and why.
#[derive(Debug, PartialEq, Eq)]
pub enum Warning {
    /// The Chord could not be read, so the Binding was dropped.
    UnreadableChord { chord: String },
    /// The Chord was fine but its Action was not, so the Binding was dropped.
    UnusableAction { chord: String, reason: String },
    /// The Chord was already bound, so the later Binding was dropped.
    DuplicateChord { chord: String },
    /// The Tap Action could not be read, so nothing happens on tap.
    UnreadableTapAction { tap: String },
}

impl Default for Config {
    /// What a first run gets: Hyper on Caps Lock, nothing bound, nothing else on.
    fn default() -> Self {
        Self {
            hyper: Key::CapsLock,
            tap: TapAction::None,
            autostart: false,
            bindings: BindingTable::empty(),
        }
    }
}

/// The file's structure could not be trusted, so nothing was loaded.
///
/// This is deliberately fatal rather than best-effort: "half my Bindings work
/// and I don't know why" is undiagnosable, whereas "nothing works and the tray
/// says why" is a five-second fix (DESIGN.md §7).
#[derive(Debug)]
pub struct ConfigError {
    pub message: String,
    /// 1-based line the problem was found on, for the settings window to show.
    /// Absent when the file could not be read at all.
    pub line: Option<usize>,
}

impl ConfigError {
    fn io(action: &str, path: &Path, error: std::io::Error) -> Self {
        Self {
            message: format!("could not {action} {}: {error}", path.display()),
            line: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawFile {
    #[serde(default)]
    hyper: RawHyper,
    #[serde(default)]
    general: RawGeneral,
    #[serde(default)]
    binding: Vec<RawBinding>,
}

/// Spanned so an unreadable Hyper Key can be reported on its own line.
#[derive(Debug, Default, Deserialize)]
struct RawHyper {
    key: Option<toml::Spanned<String>>,
    tap: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawGeneral {
    #[serde(default)]
    autostart: bool,
}

#[derive(Debug, Deserialize)]
struct RawBinding {
    chord: String,
    /// Left unresolved on purpose. Deserialising the Action here would make one
    /// bad Binding fail the whole file; resolving it per Binding lets a single
    /// unusable entry be skipped and reported instead (DESIGN.md §7).
    action: toml::Value,
}

impl Config {
    pub fn parse(text: &str) -> Result<Loaded, ConfigError> {
        let raw: RawFile = toml::from_str(text).map_err(|error| ConfigError {
            line: error.span().map(|span| line_of(text, span.start)),
            message: error.message().to_string(),
        })?;

        let mut bindings = BindingTable::empty();
        let mut warnings = Vec::new();

        let hyper = match &raw.hyper.key {
            None => Key::CapsLock,
            Some(spanned) => spanned.get_ref().parse().map_err(|()| ConfigError {
                message: format!("unknown key `{}`", spanned.get_ref()),
                line: Some(line_of(text, spanned.span().start)),
            })?,
        };

        let tap = match &raw.hyper.tap {
            None => TapAction::None,
            Some(name) => name.parse().unwrap_or_else(|()| {
                warnings.push(Warning::UnreadableTapAction { tap: name.clone() });
                TapAction::None
            }),
        };

        for entry in raw.binding {
            let Ok(chord) = entry.chord.parse::<Chord>() else {
                warnings.push(Warning::UnreadableChord { chord: entry.chord });
                continue;
            };

            match entry
                .action
                .try_into::<Action>()
                .map_err(|error| error.message().to_string())
                .and_then(|action| action.check().map(|()| action))
            {
                Ok(action) => {
                    if !bindings.insert(chord, action) {
                        warnings.push(Warning::DuplicateChord { chord: entry.chord });
                    }
                }
                Err(reason) => warnings.push(Warning::UnusableAction {
                    chord: entry.chord,
                    reason,
                }),
            }
        }

        Ok(Loaded {
            config: Config {
                hyper,
                tap,
                autostart: raw.general.autostart,
                bindings,
            },
            warnings,
        })
    }
}

/// The 1-based line containing the byte at `offset`.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].lines().count().max(1)
}

#[derive(Debug, Serialize)]
struct WrittenFile<'a> {
    hyper: WrittenHyper,
    general: WrittenGeneral,
    binding: Vec<WrittenBinding<'a>>,
}

#[derive(Debug, Serialize)]
struct WrittenHyper {
    key: String,
    tap: String,
}

#[derive(Debug, Serialize)]
struct WrittenGeneral {
    autostart: bool,
}

#[derive(Debug, Serialize)]
struct WrittenBinding<'a> {
    chord: String,
    action: &'a Action,
}

impl Config {
    /// The file this Config would be saved as.
    ///
    /// The settings window rewrites the file on every change (DESIGN.md §6), so
    /// the output has to be byte-stable: Bindings are emitted in key-table
    /// order rather than map order.
    pub fn to_toml(&self) -> String {
        let file = WrittenFile {
            hyper: WrittenHyper {
                key: self.hyper.to_string(),
                tap: self.tap.to_string(),
            },
            general: WrittenGeneral {
                autostart: self.autostart,
            },
            binding: self
                .bindings
                .sorted()
                .into_iter()
                .map(|(chord, action)| WrittenBinding {
                    chord: chord.to_string(),
                    action,
                })
                .collect(),
        };

        toml::to_string_pretty(&file).expect("a Config is always representable as TOML")
    }
}

impl Config {
    /// Where the config lives on this platform.
    ///
    /// `None` only if the platform has no home directory to speak of.
    pub fn default_path() -> Option<PathBuf> {
        // `BaseDirs` rather than `ProjectDirs`: the latter inserts an extra
        // `config` segment on Windows, giving `footman\config\config.toml`
        // where DESIGN.md §6 asks for `footman\config.toml`.
        BaseDirs::new().map(|dirs| dirs.config_dir().join("footman").join("config.toml"))
    }

    /// Loads the config, writing the defaults out first if there is no file.
    ///
    /// A first run therefore leaves a file the user can open and edit, rather
    /// than an invisible default (DESIGN.md §7).
    pub fn load_or_create(path: &Path) -> Result<Loaded, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Config::default();
                config.save(path)?;
                Ok(Loaded {
                    config,
                    warnings: Vec::new(),
                })
            }
            Err(error) => Err(ConfigError::io("read", path, error)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::io("create", parent, e))?;
        }
        fs::write(path, self.to_toml()).map_err(|e| ConfigError::io("write", path, e))
    }
}
