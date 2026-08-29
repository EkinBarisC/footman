//! Inferring whether the keyboard hook is still installed.
//!
//! Windows uninstalls a low-level hook whose callback overruns
//! `LowLevelHooksTimeout` and reports nothing — the process keeps running and
//! silently stops seeing keys (DESIGN.md §9.1). Nor is there an API to ask
//! whether a hook is still in place. So liveness is inferred from a
//! disagreement: the system recorded input that our hook never saw.
//!
//! Everything here takes tick counts as arguments rather than reading a clock,
//! so the policy is testable without an operating system.

/// What the evidence says about the hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Alive,
    /// Input happened that we did not see. Reinstall.
    Dead,
}

/// Watches for the gap between what the system saw and what we saw.
#[derive(Debug)]
pub struct HookWatch {
    /// How long we tolerate seeing nothing before the comparison means anything.
    quiet_ms: u64,
    /// Tick of the last key event our hook observed, or of the last reinstall.
    last_seen: u64,
}

impl HookWatch {
    pub fn new(quiet_ms: u64, now: u64) -> Self {
        Self {
            quiet_ms,
            last_seen: now,
        }
    }

    /// Our hook saw a key event, so it is demonstrably alive.
    pub fn saw_key(&mut self, now: u64) {
        self.last_seen = now;
    }

    /// The hook was reinstalled, so earlier evidence no longer applies.
    pub fn reinstalled(&mut self, now: u64) {
        self.last_seen = now;
    }

    /// `system_last_input` is the tick of the last input of any kind, as
    /// `GetLastInputInfo` reports it.
    ///
    /// It counts mouse input too, which our keyboard hook would never see, so a
    /// spell of mouse-only work reads as death. That false positive is accepted:
    /// its only cost is an unnecessary reinstall, which is invisible and takes
    /// microseconds — whereas the false *negative* is Footman sitting there
    /// looking fine and responding to nothing.
    pub fn check(&self, now: u64, system_last_input: u64) -> Health {
        let quiet_for_long_enough = now.saturating_sub(self.last_seen) >= self.quiet_ms;
        let system_saw_more_than_us = system_last_input > self.last_seen;

        if quiet_for_long_enough && system_saw_more_than_us {
            Health::Dead
        } else {
            Health::Alive
        }
    }
}

/// Reads a 32-bit tick count on the 64-bit clock.
///
/// `GetLastInputInfo` reports the shorter one and `GetTickCount64` the longer,
/// and they agree only until the shorter wraps — about seven weeks after a
/// machine is switched on. Compared raw after that, the system's last input
/// looks like it happened at the dawn of the clock, `check` never sees the
/// system get ahead of us, and the watchdog stops being able to notice a dead
/// hook at all. Which is precisely the machine that has been running for weeks
/// without a restart: the one that needs it.
pub fn same_clock(now: u64, short: u32) -> u64 {
    let lifted = (now & !0xFFFF_FFFF) | u64::from(short);
    // The reading is of something that has already happened, so a value in the
    // future means it belongs to the turn of the clock before this one.
    if lifted > now {
        lifted - (1 << 32)
    } else {
        lifted
    }
}
