//! Durations and the focus/break cycle (T-01, T-02).

const MIN: i64 = 60_000;

/// Smallest and largest allowed phase length. The bounds are a guess; change them in one place if needed.
pub const MIN_PHASE_MS: i64 = MIN;
pub const MAX_PHASE_MS: i64 = 12 * 60 * MIN;
/// Most focus rounds before a long break that the settings accept (also a guess).
pub const MAX_ROUNDS: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// A duration is outside `MIN_PHASE_MS..=MAX_PHASE_MS`.
    DurationOutOfRange,
    /// Rounds before a long break must be `1..=MAX_ROUNDS`.
    RoundsOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakKind {
    Short,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    focus_ms: i64,
    short_break_ms: i64,
    long_break_ms: i64,
    rounds_before_long: u32,
}

impl Default for Config {
    /// 25 / 5 / 15 minutes, long break after every 4th focus round.
    fn default() -> Self {
        Self {
            focus_ms: 25 * MIN,
            short_break_ms: 5 * MIN,
            long_break_ms: 15 * MIN,
            rounds_before_long: 4,
        }
    }
}

impl Config {
    pub fn new(
        focus_ms: i64,
        short_break_ms: i64,
        long_break_ms: i64,
        rounds_before_long: u32,
    ) -> Result<Self, ConfigError> {
        let in_range = |ms: i64| (MIN_PHASE_MS..=MAX_PHASE_MS).contains(&ms);
        if ![focus_ms, short_break_ms, long_break_ms]
            .into_iter()
            .all(in_range)
        {
            return Err(ConfigError::DurationOutOfRange);
        }
        if !(1..=MAX_ROUNDS).contains(&rounds_before_long) {
            return Err(ConfigError::RoundsOutOfRange);
        }
        Ok(Self {
            focus_ms,
            short_break_ms,
            long_break_ms,
            rounds_before_long,
        })
    }

    pub fn focus_ms(&self) -> i64 {
        self.focus_ms
    }

    pub fn rounds_before_long(&self) -> u32 {
        self.rounds_before_long
    }

    pub fn break_ms(&self, kind: BreakKind) -> i64 {
        match kind {
            BreakKind::Short => self.short_break_ms,
            BreakKind::Long => self.long_break_ms,
        }
    }

    /// Which break follows the focus round that just completed. `completed_focus` counts finished focus
    /// rounds in the current cycle, including that one. Every `rounds_before_long`-th round earns a long break.
    pub fn break_after(&self, completed_focus: u32) -> BreakKind {
        if completed_focus > 0 && completed_focus % self.rounds_before_long == 0 {
            BreakKind::Long
        } else {
            BreakKind::Short
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_25_5_15_every_4() {
        let c = Config::default();
        assert_eq!(c.focus_ms(), 25 * MIN);
        assert_eq!(c.break_ms(BreakKind::Short), 5 * MIN);
        assert_eq!(c.break_ms(BreakKind::Long), 15 * MIN);
        assert_eq!(c.rounds_before_long(), 4);
    }

    #[test]
    fn long_break_after_every_fourth_round() {
        let c = Config::default();
        let kinds: Vec<_> = (1..=9).map(|n| c.break_after(n)).collect();
        use BreakKind::{Long, Short};
        assert_eq!(
            kinds,
            [Short, Short, Short, Long, Short, Short, Short, Long, Short]
        );
    }

    #[test]
    fn one_round_means_always_long() {
        let c = Config::new(25 * MIN, 5 * MIN, 15 * MIN, 1).expect("config");
        assert_eq!(c.break_after(1), BreakKind::Long);
        assert_eq!(c.break_after(2), BreakKind::Long);
    }

    #[test]
    fn zero_completed_rounds_gets_a_short_break() {
        assert_eq!(Config::default().break_after(0), BreakKind::Short);
    }

    #[test]
    fn validates_ranges() {
        assert_eq!(
            Config::new(MIN - 1, 5 * MIN, 15 * MIN, 4),
            Err(ConfigError::DurationOutOfRange)
        );
        assert_eq!(
            Config::new(25 * MIN, MAX_PHASE_MS + 1, 15 * MIN, 4),
            Err(ConfigError::DurationOutOfRange)
        );
        assert_eq!(
            Config::new(25 * MIN, 5 * MIN, 15 * MIN, 0),
            Err(ConfigError::RoundsOutOfRange)
        );
        assert_eq!(
            Config::new(25 * MIN, 5 * MIN, 15 * MIN, MAX_ROUNDS + 1),
            Err(ConfigError::RoundsOutOfRange)
        );
        assert!(Config::new(MIN, MIN, MIN, 1).is_ok());
        assert!(Config::new(MAX_PHASE_MS, MAX_PHASE_MS, MAX_PHASE_MS, MAX_ROUNDS).is_ok());
    }
}
