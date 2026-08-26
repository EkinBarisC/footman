//! The Dispatcher: the thread that carries Effects out.
//!
//! It exists so the hook callback does not. Every Effect here costs far more
//! than the callback's budget — enumerating windows, COM calls, starting
//! processes — and a callback that overruns is silently uninstalled
//! (DESIGN.md §9.1). The channel between them is the whole point.

use std::sync::mpsc::Receiver;

use super::apps;
use crate::{Action, Effect};

/// Runs until the hook thread goes away and the channel closes. Blocks; call it
/// on a thread of its own.
pub fn dispatch(inbox: Receiver<Effect>) {
    apps::init_thread();

    for effect in inbox {
        let failure = match effect {
            Effect::Run(Action::App { ref id }) => apps::focus(id).err(),
            // Slice 5 (DESIGN.md §12). Until then, saying so beats silence.
            ref other => Some(format!("{other:?} is not implemented yet")),
        };

        // An Action that fails must not take the process with it: the keyboard
        // comes first (DESIGN.md §7), and a dead Dispatcher would mean a
        // Chord that suppresses and does nothing, for ever. The tray will
        // report this properly in slice 6.
        if let Some(message) = failure {
            eprintln!("footman: {message}");
        }
    }
}
