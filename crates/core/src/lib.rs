//! Pure timer, state machine, ids and event log. No I/O.

pub mod clock;
pub mod config;
pub mod machine;
pub mod timer;

pub use clock::{Clock, FakeClock, SystemClock};
pub use config::{BreakKind, Config, ConfigError};
pub use machine::{Effect, Event, Outcome, PausedFrom, Rejected, Rules, State, transition};
pub use timer::{EXTEND_STEP_MS, Timer, TimerError};
