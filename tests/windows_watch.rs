//! When to conclude the keyboard hook has been silently uninstalled.
//!
//! Windows drops a low-level hook whose callback overruns `LowLevelHooksTimeout`
//! and says nothing: no error, no log. The application keeps running and simply
//! stops seeing keys (DESIGN.md §9.1). There is no API to ask whether a hook is
//! still installed, so liveness has to be inferred.
//!
//! The inference is kept pure — it takes tick counts rather than reading a clock
//! — so the policy is testable without an operating system or a stopwatch.

#![cfg(windows)]

use footman::windows::{Health, HookWatch, same_clock};

const QUIET: u64 = 30_000;

#[test]
fn a_hook_that_is_seeing_keys_is_alive() {
    let mut watch = HookWatch::new(QUIET, 0);
    watch.saw_key(1_000);

    // The system agrees input happened, and we saw it.
    assert_eq!(watch.check(2_000, 1_000), Health::Alive);
}

/// An idle machine is not a broken hook. Nothing has happened that we failed to
/// see, so there is nothing to conclude.
#[test]
fn silence_on_an_idle_machine_is_not_death() {
    let mut watch = HookWatch::new(QUIET, 0);
    watch.saw_key(1_000);

    assert_eq!(watch.check(1_000 + QUIET + 1, 1_000), Health::Alive);
}

/// The actual signal: the system recorded input after the last event our hook
/// saw, and we have been quiet throughout. We missed something.
#[test]
fn input_we_never_saw_means_the_hook_is_gone() {
    let mut watch = HookWatch::new(QUIET, 0);
    watch.saw_key(1_000);

    assert_eq!(watch.check(1_000 + QUIET + 1, 5_000), Health::Dead);
}

/// Before the quiet period is up, a gap is just a gap — the user may simply
/// have paused mid-sentence.
#[test]
fn a_short_gap_is_never_death() {
    let mut watch = HookWatch::new(QUIET, 0);
    watch.saw_key(1_000);

    assert_eq!(watch.check(1_000 + QUIET - 1, 5_000), Health::Alive);
}

/// After reinstalling, the watch starts again from that moment rather than
/// immediately re-reporting the same stale evidence.
#[test]
fn reinstalling_clears_the_suspicion() {
    let mut watch = HookWatch::new(QUIET, 0);
    watch.saw_key(1_000);
    let now = 1_000 + QUIET + 1;
    assert_eq!(watch.check(now, 5_000), Health::Dead);

    watch.reinstalled(now);

    assert_eq!(watch.check(now + 1, 5_000), Health::Alive);
}

/// `GetLastInputInfo` reports a 32-bit tick count and `GetTickCount64` a
/// 64-bit one. Comparing them raw works only until the shorter clock wraps,
/// about seven weeks after a machine is switched on, after which the watchdog
/// would decide the system had seen no input since 1970 and never reinstall a
/// dead hook again. So the short reading is lifted into the long one's domain
/// before anything is compared.
#[test]
fn a_short_tick_is_read_on_the_long_clock() {
    assert_eq!(same_clock(5_000, 4_000), 4_000);
}

#[test]
fn a_short_tick_after_a_wrap_is_still_in_the_past() {
    const WRAP: u64 = 1 << 32;
    // A second past the wrap; the last input was a second before it.
    assert_eq!(same_clock(WRAP + 1_000, 0xFFFF_FC18), WRAP - 1_000);
}

#[test]
fn the_two_clocks_can_agree_exactly() {
    assert_eq!(same_clock(4_000, 4_000), 4_000);
}
