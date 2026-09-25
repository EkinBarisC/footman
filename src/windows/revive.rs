//! Deciding whether the tray and settings window are rebuilt after they die.
//!
//! The user interface belongs to dependencies — eframe, its OpenGL context,
//! `winit` — and they can fail in ways Footman cannot prevent: resuming a
//! hybrid-graphics laptop from hibernation loses the GL context, and the next
//! resize of even a hidden window panics. The hook does not depend on any of
//! that (ADR-0007), so losing the window must not cost the keyboard. It is
//! rebuilt instead (ADR-0008).
//!
//! But not forever. A window that dies again the moment it is built is not a
//! wake-up; it is a machine on which it cannot exist, and rebuilding it in a
//! loop would be a process spinning where the user cannot see it.
//!
//! Like `HookWatch`, this takes tick counts rather than reading a clock, so the
//! policy is testable without an operating system.

/// What to do about a window that has just died.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// Build it again after this many milliseconds.
    After(u64),
    /// Stop trying: it has died too often, too recently.
    GiveUp,
}

/// Remembers recent deaths and rules on each new one.
#[derive(Debug)]
pub struct Revival {
    /// How many deaths inside `span_ms` are tolerated.
    limit: usize,
    span_ms: u64,
    /// Ticks of the deaths still inside the span.
    deaths: Vec<u64>,
}

/// The first rebuild waits this long: long enough for a graphics driver that
/// has just come back from hibernation to finish arriving.
const FIRST_WAIT_MS: u64 = 2_000;

/// No wait grows past this, however many deaths preceded it.
///
/// A minute rather than the half one this began as: measured on the machine
/// this was written for, a window rebuilt too soon after the laptop wakes
/// fails with "found no glutin configs matching the template" — there is no
/// GPU to give it one yet — and the graphics stack took some thirty seconds to
/// come back. Waiting is the whole remedy for that.
const LONGEST_WAIT_MS: u64 = 60_000;

impl Revival {
    pub fn new(limit: usize, span_ms: u64) -> Self {
        Self {
            limit,
            span_ms,
            deaths: Vec::new(),
        }
    }

    /// The window died at `now`.
    ///
    /// Deaths are forgotten once they are older than the span. A laptop that
    /// sleeps every night loses its window every night, and that is a window to
    /// rebuild every night rather than a count to exhaust by Friday.
    pub fn died(&mut self, now: u64) -> Recovery {
        self.deaths
            .retain(|&then| now.saturating_sub(then) < self.span_ms);
        self.deaths.push(now);

        let recent = self.deaths.len();
        if recent > self.limit {
            return Recovery::GiveUp;
        }

        // Doubling, so a window that keeps dying is given more time each round
        // for whatever is wrong underneath it to settle.
        let doublings = u32::try_from(recent - 1).unwrap_or(u32::MAX);
        let wait = FIRST_WAIT_MS
            .checked_shl(doublings)
            .unwrap_or(LONGEST_WAIT_MS)
            .min(LONGEST_WAIT_MS);
        Recovery::After(wait)
    }
}
