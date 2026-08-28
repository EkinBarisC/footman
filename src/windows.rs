//! The Windows Shell.
//!
//! Everything here is specific to Windows and selected by `cfg` rather than
//! hidden behind a trait — see ADR-0002.

mod apps;
mod dispatch;
mod hook;
mod identity;
mod keys;
mod watch;

#[cfg(feature = "integration-test")]
pub mod test_support;

pub use apps::{focus, hidden_by_cloaking, init_thread, survey};
pub use dispatch::dispatch;
pub use hook::{FOOTMAN_SIGNATURE, HookError, run};
pub use identity::{Launch, launch_of, matches};
pub use keys::{key_to_vk, vk_to_key};
pub use watch::{Health, HookWatch};
