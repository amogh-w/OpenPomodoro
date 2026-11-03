//! Timestamp-based phase timer (T-03, T-04, T-05).
//!
//! Time is never counted; it is computed from timestamps, so a reload, sleep or second tab always agrees.
//!
//! ```text
//! running: elapsed = now - start - pause_buffer
//! paused:  elapsed = pause_ts - start - pause_buffer
//! ```
//!
//! The phase has ended when `elapsed >= total`. Everything past that is overflow.

use std::fmt;

/// 5 minutes: the step for extending or shrinking a phase (T-05).
pub const EXTEND_STEP_MS: i64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
    /// A phase needs a total greater than zero.
    InvalidTotal,
    AlreadyPaused,
    NotPaused,
}

impl TimerError {
    /// Stable code shared with the JSON test vectors.
    pub fn code(self) -> &'static str {
        match self {
            TimerError::InvalidTotal => "invalid_total",
            TimerError::AlreadyPaused => "already_paused",
            TimerError::NotPaused => "not_paused",
        }
    }
}

impl fmt::Display for TimerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for TimerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    start: i64,
    total: i64,
    pause_buffer: i64,
    pause_ts: Option<i64>,
}

impl Timer {
    pub fn new(start_ms: i64, total_ms: i64) -> Result<Self, TimerError> {
        if total_ms <= 0 {
            return Err(TimerError::InvalidTotal);
        }
        Ok(Self {
            start: start_ms,
            total: total_ms,
            pause_buffer: 0,
            pause_ts: None,
        })
    }

    /// Restore a timer from stored fields. No checks: a shrunk phase may legitimately have a total of 0.
    pub fn from_parts(
        start_ms: i64,
        total_ms: i64,
        pause_buffer_ms: i64,
        pause_ts: Option<i64>,
    ) -> Self {
        Self {
            start: start_ms,
            total: total_ms,
            pause_buffer: pause_buffer_ms,
            pause_ts,
        }
    }

    pub fn start(&self) -> i64 {
        self.start
    }

    /// When the timer was paused, if it is paused.
    pub fn pause_ts(&self) -> Option<i64> {
        self.pause_ts
    }

    /// The instant the phase reaches (or reached) its total, assuming it keeps running from now: the
    /// start plus the total plus everything spent paused. Meaningful for a running timer.
    pub fn end_instant(&self) -> i64 {
        self.start + self.total + self.pause_buffer
    }

    pub fn total(&self) -> i64 {
        self.total
    }

    pub fn pause_buffer(&self) -> i64 {
        self.pause_buffer
    }

    pub fn is_paused(&self) -> bool {
        self.pause_ts.is_some()
    }

    /// Time the phase has actually run. Never negative: if the clock was set back so that `now` is
    /// before the start, this reads 0 (see [`Timer::clock_went_back`] to log that).
    pub fn elapsed(&self, now_ms: i64) -> i64 {
        let at = self.pause_ts.unwrap_or(now_ms);
        (at - self.start - self.pause_buffer).max(0)
    }

    pub fn remaining(&self, now_ms: i64) -> i64 {
        (self.total - self.elapsed(now_ms)).max(0)
    }

    pub fn overflow(&self, now_ms: i64) -> i64 {
        (self.elapsed(now_ms) - self.total).max(0)
    }

    /// True at exactly `elapsed == total` and after.
    pub fn is_ended(&self, now_ms: i64) -> bool {
        self.elapsed(now_ms) >= self.total
    }

    /// True when `now` is earlier than the timer's own timestamps, so the caller can write a diagnostic.
    pub fn clock_went_back(&self, now_ms: i64) -> bool {
        now_ms < self.start || self.pause_ts.is_some_and(|p| now_ms < p)
    }

    pub fn pause(&mut self, now_ms: i64) -> Result<(), TimerError> {
        if self.pause_ts.is_some() {
            return Err(TimerError::AlreadyPaused);
        }
        self.pause_ts = Some(now_ms);
        Ok(())
    }

    /// Resume. The paused span joins `pause_buffer`; if the clock went back meanwhile the span counts as 0.
    pub fn resume(&mut self, now_ms: i64) -> Result<(), TimerError> {
        let Some(paused_at) = self.pause_ts.take() else {
            return Err(TimerError::NotPaused);
        };
        self.pause_buffer += (now_ms - paused_at).max(0);
        Ok(())
    }

    /// Extend (`delta > 0`) or shrink (`delta < 0`) the phase. `elapsed` is never reset, so recorded
    /// durations stay true.
    ///
    /// - Before the end: `total += delta`; shrinking to or below `elapsed` ends the phase now (`total = elapsed`).
    /// - At or past the end (overflow): a positive `delta` starts a fresh countdown of `delta` from now
    ///   (`total = elapsed + delta`); a shrink does nothing because the phase has already ended.
    pub fn extend(&mut self, delta_ms: i64, now_ms: i64) {
        let elapsed = self.elapsed(now_ms);
        if elapsed >= self.total {
            if delta_ms > 0 {
                self.total = elapsed + delta_ms;
            }
        } else {
            self.total = (self.total + delta_ms).max(elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{Clock, FakeClock};

    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;

    fn timer(total_min: i64, clock: &FakeClock) -> Timer {
        Timer::new(clock.now_ms(), total_min * MIN).expect("valid timer")
    }

    #[test]
    fn parts_round_trip_and_end_instant() {
        let t = Timer::from_parts(1_000, 5 * MIN, 2 * MIN, Some(9_000));
        assert_eq!(
            (t.start(), t.total(), t.pause_buffer(), t.pause_ts()),
            (1_000, 5 * MIN, 2 * MIN, Some(9_000))
        );
        assert_eq!(t.end_instant(), 1_000 + 7 * MIN);
        assert_eq!(
            Timer::from_parts(0, 0, 0, None).total(),
            0,
            "a total of 0 is allowed when restoring"
        );
    }

    #[test]
    fn rejects_non_positive_total() {
        assert_eq!(Timer::new(0, 0), Err(TimerError::InvalidTotal));
        assert_eq!(Timer::new(0, -1), Err(TimerError::InvalidTotal));
    }

    #[test]
    fn ends_exactly_at_total() {
        let clock = FakeClock::new(1_000_000);
        let t = timer(25, &clock);
        clock.advance(25 * MIN - 1);
        assert!(!t.is_ended(clock.now_ms()));
        clock.advance(1);
        assert!(t.is_ended(clock.now_ms()));
        assert_eq!(t.overflow(clock.now_ms()), 0);
    }

    #[test]
    fn tab_hidden_for_an_hour_catches_up() {
        let clock = FakeClock::new(0);
        let t = timer(25, &clock);
        clock.advance(HOUR);
        assert_eq!(t.elapsed(clock.now_ms()), HOUR);
        assert_eq!(t.overflow(clock.now_ms()), HOUR - 25 * MIN);
    }

    #[test]
    fn pause_freezes_and_resume_continues() {
        let clock = FakeClock::new(0);
        let mut t = timer(10, &clock);
        clock.advance(3 * MIN);
        t.pause(clock.now_ms()).expect("pause");
        clock.advance(5 * MIN);
        assert_eq!(t.elapsed(clock.now_ms()), 3 * MIN);
        t.resume(clock.now_ms()).expect("resume");
        assert_eq!(t.pause_buffer(), 5 * MIN);
        clock.advance(2 * MIN);
        assert_eq!(t.elapsed(clock.now_ms()), 5 * MIN);
    }

    #[test]
    fn pause_state_errors() {
        let mut t = Timer::new(0, MIN).expect("timer");
        assert_eq!(t.resume(0), Err(TimerError::NotPaused));
        t.pause(0).expect("pause");
        assert_eq!(t.pause(1), Err(TimerError::AlreadyPaused));
    }

    #[test]
    fn pause_during_overflow_freezes_overflow() {
        let clock = FakeClock::new(0);
        let mut t = timer(5, &clock);
        clock.advance(7 * MIN);
        t.pause(clock.now_ms()).expect("pause");
        clock.advance(HOUR);
        assert_eq!(t.overflow(clock.now_ms()), 2 * MIN);
        assert!(t.is_ended(clock.now_ms()));
    }

    #[test]
    fn extend_and_shrink_before_the_end() {
        let clock = FakeClock::new(0);
        let mut t = timer(25, &clock);
        clock.advance(10 * MIN);
        t.extend(EXTEND_STEP_MS, clock.now_ms());
        assert_eq!(t.remaining(clock.now_ms()), 20 * MIN);
        t.extend(-EXTEND_STEP_MS, clock.now_ms());
        t.extend(-EXTEND_STEP_MS, clock.now_ms());
        assert_eq!(t.remaining(clock.now_ms()), 10 * MIN);
        assert_eq!(t.elapsed(clock.now_ms()), 10 * MIN);
    }

    #[test]
    fn shrink_past_zero_ends_now_without_overflow() {
        let clock = FakeClock::new(0);
        let mut t = timer(25, &clock);
        clock.advance(20 * MIN);
        t.extend(-10 * MIN, clock.now_ms());
        assert!(t.is_ended(clock.now_ms()));
        assert_eq!(t.overflow(clock.now_ms()), 0);
        assert_eq!(t.total(), 20 * MIN);
    }

    #[test]
    fn extend_in_overflow_starts_a_fresh_countdown() {
        let clock = FakeClock::new(0);
        let mut t = timer(5, &clock);
        clock.advance(8 * MIN);
        t.extend(EXTEND_STEP_MS, clock.now_ms());
        assert!(!t.is_ended(clock.now_ms()));
        assert_eq!(t.remaining(clock.now_ms()), 5 * MIN);
        assert_eq!(t.elapsed(clock.now_ms()), 8 * MIN, "elapsed is never reset");
    }

    #[test]
    fn shrink_in_overflow_is_a_no_op() {
        let clock = FakeClock::new(0);
        let mut t = timer(5, &clock);
        clock.advance(8 * MIN);
        t.extend(-EXTEND_STEP_MS, clock.now_ms());
        assert_eq!(t.overflow(clock.now_ms()), 3 * MIN);
    }

    #[test]
    fn extend_at_the_exact_end_counts_as_overflow_case() {
        let clock = FakeClock::new(0);
        let mut t = timer(5, &clock);
        clock.advance(5 * MIN);
        t.extend(EXTEND_STEP_MS, clock.now_ms());
        assert_eq!(t.remaining(clock.now_ms()), 5 * MIN);
    }

    #[test]
    fn extend_while_paused_keeps_it_paused() {
        let clock = FakeClock::new(0);
        let mut t = timer(10, &clock);
        clock.advance(4 * MIN);
        t.pause(clock.now_ms()).expect("pause");
        clock.advance(2 * MIN);
        t.extend(EXTEND_STEP_MS, clock.now_ms());
        assert!(t.is_paused());
        assert_eq!(t.remaining(clock.now_ms()), 11 * MIN);
    }

    #[test]
    fn clock_before_start_reads_zero_elapsed() {
        let clock = FakeClock::new(1_000_000);
        let t = timer(5, &clock);
        clock.set(900_000);
        assert_eq!(t.elapsed(clock.now_ms()), 0);
        assert_eq!(t.remaining(clock.now_ms()), 5 * MIN);
        assert!(t.clock_went_back(clock.now_ms()));
    }

    #[test]
    fn clock_moving_back_while_paused_does_not_shrink_the_buffer() {
        let clock = FakeClock::new(0);
        let mut t = timer(10, &clock);
        clock.advance(100_000);
        t.pause(clock.now_ms()).expect("pause");
        clock.set(90_000);
        assert!(t.clock_went_back(clock.now_ms()));
        t.resume(clock.now_ms()).expect("resume");
        assert_eq!(t.pause_buffer(), 0);
        assert_eq!(t.elapsed(clock.now_ms()), 90_000);
    }

    #[test]
    fn sleep_while_running_ends_the_phase_on_wake() {
        let clock = FakeClock::new(0);
        let t = timer(25, &clock);
        clock.advance(8 * HOUR);
        assert!(t.is_ended(clock.now_ms()));
        assert_eq!(t.overflow(clock.now_ms()), 8 * HOUR - 25 * MIN);
    }

    #[test]
    fn sleep_while_paused_changes_nothing() {
        let clock = FakeClock::new(0);
        let mut t = timer(25, &clock);
        clock.advance(10 * MIN);
        t.pause(clock.now_ms()).expect("pause");
        clock.advance(9 * HOUR);
        assert_eq!(t.elapsed(clock.now_ms()), 10 * MIN);
        assert!(!t.is_ended(clock.now_ms()));
    }
}
