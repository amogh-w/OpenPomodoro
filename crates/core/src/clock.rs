//! Injectable time source. All times are Unix milliseconds (`i64`).

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock {
    fn now_ms(&self) -> i64;
}

/// Wall clock. A system clock set before 1970 reads as 0 rather than panicking.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok())
            .unwrap_or(0)
    }
}

/// Test clock that only moves when told to, including backwards.
pub struct FakeClock(AtomicI64);

impl FakeClock {
    pub fn new(start_ms: i64) -> Self {
        Self(AtomicI64::new(start_ms))
    }

    pub fn set(&self, ms: i64) {
        self.0.store(ms, Ordering::Relaxed);
    }

    pub fn advance(&self, delta_ms: i64) {
        self.0.fetch_add(delta_ms, Ordering::Relaxed);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_moves_forward_and_back() {
        let clock = FakeClock::new(1_000);
        clock.advance(500);
        assert_eq!(clock.now_ms(), 1_500);
        clock.set(200);
        assert_eq!(clock.now_ms(), 200);
    }

    #[test]
    fn system_clock_is_after_2020() {
        assert!(SystemClock.now_ms() > 1_577_836_800_000);
    }
}
