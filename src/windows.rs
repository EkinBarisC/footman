//! The Windows Shell.
//!
//! Everything here is specific to Windows and selected by `cfg` rather than
//! hidden behind a trait — see ADR-0002.

mod apps;
mod commands;
mod desktops;
mod dispatch;
mod hook;
mod identity;
mod keys;
mod synthetic;
mod watch;

#[cfg(feature = "integration-test")]
pub mod test_support;

pub use apps::{focus, hidden_by_cloaking, init_thread, survey};
pub use commands::{CREATE_NEW_CONSOLE, CREATE_NO_WINDOW, creation_flags, run as run_command};
pub use desktops::{Desktops, Move, desktops_from, position, steps_to, switch_to};
pub use dispatch::dispatch;
pub use hook::{FOOTMAN_SIGNATURE, Hook, HookError, spawn};
pub use identity::{Launch, launch_of, matches};
pub use keys::{key_to_vk, vk_to_key};
pub use watch::{Health, HookWatch};
