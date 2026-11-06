//! A short in-memory log of things the admin may need to see: hooks and webhooks that failed, commands the timer
//! refused. Newest last, capped, gone on restart. Never holds passwords, tokens or anything a person typed.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lines kept; older ones fall off.
pub const CAPACITY: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Line {
    /// Unix milliseconds.
    pub at: i64,
    pub area: &'static str,
    pub message: String,
}

fn store() -> &'static Mutex<VecDeque<Line>> {
    static STORE: OnceLock<Mutex<VecDeque<Line>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(VecDeque::with_capacity(CAPACITY)))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

/// Add a line (and echo it to the server's standard error, where a service manager collects it).
pub fn record(area: &'static str, message: impl Into<String>) {
    let message = message.into();
    eprintln!("{area}: {message}");
    let mut lines = store().lock().unwrap_or_else(|p| p.into_inner());
    if lines.len() == CAPACITY {
        lines.pop_front();
    }
    lines.push_back(Line {
        at: now_ms(),
        area,
        message,
    });
}

/// The kept lines, oldest first.
pub fn lines() -> Vec<Line> {
    store()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_kept_in_order_and_capped() {
        for i in 0..(CAPACITY + 20) {
            record("test-cap", format!("line {i}"));
        }
        let mine: Vec<_> = lines()
            .into_iter()
            .filter(|l| l.area == "test-cap")
            .collect();
        assert!(mine.len() <= CAPACITY);
        assert_eq!(
            mine.last().map(|l| l.message.as_str()),
            Some(&*format!("line {}", CAPACITY + 19))
        );
        assert!(mine.windows(2).all(|w| w[0].at <= w[1].at));
    }
}
