//! The Windows Shell.
//!
//! Everything here is specific to Windows and selected by `cfg` rather than
//! hidden behind a trait — see ADR-0002.

mod hook;
mod keys;
mod watch;

#[cfg(feature = "integration-test")]
pub mod test_support;

pub use hook::{FOOTMAN_SIGNATURE, HookError, run};
pub use keys::{key_to_vk, vk_to_key};
pub use watch::{Health, HookWatch};
