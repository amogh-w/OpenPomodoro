//! Runs a user's timer: turns a command ("start focus", "pause", "stop", ...) into the next timer state and the
//! session records it completes. The state machine and the timer rules come from `op-core`; this module only
//! feeds them stored state and writes the result.
//!
//! Every timer event is written with a server-made id, so the timer row always moves forward no matter how old
//! the client's own timestamp is. A command's own id is only an idempotency key.

use op_core::config::{MAX_PHASE_MS, MIN_PHASE_MS};
use op_core::{
    Applied, BreakKind, EXTEND_STEP_MS, Effect, Entity, Event, EventRecord, IdGen, Op, Outcome,
    Rejected, State, Timer, transition,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::db::{Db, DbError};
use crate::settings::Settings;
use crate::validate;

/// A command may claim a time slightly ahead of the server clock (clock skew), but not more than this.
const MAX_TIME_SKEW_MS: i64 = 60_000;
/// Extend and shrink come in 5-minute steps, at most an hour at a time.
const MAX_EXTEND_STEPS: i64 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseKind {
    Focus,
    ShortBreak,
    LongBreak,
}

impl PhaseKind {
    pub fn code(self) -> &'static str {
        match self {
            PhaseKind::Focus => "focus",
            PhaseKind::ShortBreak => "short_break",
            PhaseKind::LongBreak => "long_break",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        match code {
            "focus" => Some(PhaseKind::Focus),
            "short_break" => Some(PhaseKind::ShortBreak),
            "long_break" => Some(PhaseKind::LongBreak),
            _ => None,
        }
    }

    fn of_break(kind: BreakKind) -> Self {
        match kind {
            BreakKind::Short => PhaseKind::ShortBreak,
            BreakKind::Long => PhaseKind::LongBreak,
        }
    }
}

/// Everything about a user's timer. Stored as the fields of their `timer/current` row.
#[derive(Debug, Clone, PartialEq)]
pub struct TimerState {
    pub state: State,
    pub kind: Option<PhaseKind>,
    pub timer: Option<Timer>,
    /// Planned focus length, kept through the breathing step.
    pub planned_ms: Option<i64>,
    pub intention: Option<String>,
    pub category: Option<String>,
    pub focus_started_at: Option<i64>,
    /// Set while in `reflect`: when the focus phase ended and how long it really ran.
    pub focus_ended_at: Option<i64>,
    pub focus_elapsed_ms: Option<i64>,
    /// Focus rounds kept since the last long break.
    pub rounds: u32,
}

impl TimerState {
    pub fn idle() -> Self {
        Self {
            state: State::Idle,
            kind: None,
            timer: None,
            planned_ms: None,
            intention: None,
            category: None,
            focus_started_at: None,
            focus_ended_at: None,
            focus_elapsed_ms: None,
            rounds: 0,
        }
    }

    /// Every field is always written (null when unset), so one event always replaces the whole state.
    pub fn to_json(&self) -> Map<String, Value> {
        let t = self.timer.as_ref();
        let mut m = Map::new();
        m.insert("state".into(), json!(self.state.code()));
        m.insert("kind".into(), json!(self.kind.map(PhaseKind::code)));
        m.insert("start".into(), json!(t.map(Timer::start)));
        m.insert("total".into(), json!(t.map(Timer::total)));
        m.insert("pause_buffer".into(), json!(t.map(Timer::pause_buffer)));
        m.insert("pause_ts".into(), json!(t.and_then(Timer::pause_ts)));
        m.insert("planned_ms".into(), json!(self.planned_ms));
        m.insert("intention".into(), json!(self.intention));
        m.insert("category".into(), json!(self.category));
        m.insert("focus_started_at".into(), json!(self.focus_started_at));
        m.insert("focus_ended_at".into(), json!(self.focus_ended_at));
        m.insert("focus_elapsed_ms".into(), json!(self.focus_elapsed_ms));
        m.insert("rounds".into(), json!(self.rounds));
        m
    }

    pub fn from_data(data: &Map<String, Value>) -> Self {
        let int = |k: &str| data.get(k).and_then(Value::as_i64);
        let text = |k: &str| data.get(k).and_then(Value::as_str).map(str::to_string);
        let state = data
            .get("state")
            .and_then(Value::as_str)
            .and_then(State::from_code)
            .unwrap_or(State::Idle);
        let timer = int("start").zip(int("total")).map(|(start, total)| {
            Timer::from_parts(
                start,
                total,
                int("pause_buffer").unwrap_or(0),
                int("pause_ts"),
            )
        });
        Self {
            state,
            kind: text("kind").as_deref().and_then(PhaseKind::from_code),
            timer,
            planned_ms: int("planned_ms"),
            intention: text("intention"),
            category: text("category"),
            focus_started_at: int("focus_started_at"),
            focus_ended_at: int("focus_ended_at"),
            focus_elapsed_ms: int("focus_elapsed_ms"),
            rounds: int("rounds")
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0),
        }
    }

    pub fn load(db: &Db, user: &str) -> Result<Self, DbError> {
        Ok(db
            .current_timer(user)?
            .map_or_else(Self::idle, |data| Self::from_data(&data)))
    }

    /// Back to idle, keeping only the round count.
    fn cleared(&self) -> Self {
        Self {
            rounds: self.rounds,
            ..Self::idle()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Command {
    /// Client-made id; makes a retry or an outbox replay harmless.
    pub id: String,
    pub name: String,
    /// When the user did it, Unix ms. Omitted means "now". The offline outbox sends the real time.
    pub at: Option<i64>,
    pub intention: Option<String>,
    pub category: Option<String>,
    /// Focus length for `init_focus` / `start_focus`; the user's setting when omitted.
    pub duration_ms: Option<i64>,
    /// Extend (positive) or shrink (negative) by whole 5-minute steps.
    pub delta_ms: Option<i64>,
    /// For `take_rest`: `short` or `long`, instead of the cycle's choice.
    pub kind: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandError {
    /// The command itself is malformed (stable code for the client).
    Invalid(&'static str),
    /// The command is well formed but not allowed right now.
    Rejected(&'static str),
    Db(String),
}

impl From<DbError> for CommandError {
    fn from(e: DbError) -> Self {
        CommandError::Db(e.to_string())
    }
}

#[derive(Debug, PartialEq)]
pub enum CommandResult {
    Applied(TimerState),
    /// This command id was processed before; nothing changed.
    Duplicate,
}

/// A finished piece of work to store as a session row.
enum Record {
    Focus {
        started_at: i64,
        ended_at: i64,
        elapsed: i64,
        planned: i64,
        intention: Option<String>,
        category: Option<String>,
    },
    Break {
        kind: PhaseKind,
        started_at: i64,
        ended_at: i64,
        elapsed: i64,
        planned: i64,
    },
}

fn clean(text: &Option<String>, max_chars: usize) -> Result<Option<String>, CommandError> {
    let Some(raw) = text else { return Ok(None) };
    let t = raw.trim();
    if t.chars().count() > max_chars || t.chars().any(|c| c.is_control() && c != '\n') {
        return Err(CommandError::Invalid("bad_args"));
    }
    Ok(if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    })
}

fn reject(r: Rejected) -> CommandError {
    CommandError::Rejected(r.code())
}

fn timer_of(state: &TimerState) -> Result<&Timer, CommandError> {
    state
        .timer
        .as_ref()
        .ok_or(CommandError::Rejected("unknown"))
}

/// Begin the breathing step, or the focus session itself, at time `at`.
fn begin_focus(next: &mut TimerState, new_state: State, at: i64, settings: &Settings) {
    next.kind = Some(PhaseKind::Focus);
    if new_state == State::Breathe {
        next.timer = Timer::new(at, settings.breathe_ms).ok();
        next.focus_started_at = None;
    } else {
        begin_session(next, at);
    }
}

fn begin_session(next: &mut TimerState, at: i64) {
    next.kind = Some(PhaseKind::Focus);
    next.timer = next
        .planned_ms
        .and_then(|planned| Timer::new(at, planned).ok());
    next.focus_started_at = Some(at);
}

fn begin_break(next: &mut TimerState, kind: BreakKind, at: i64, settings: &Settings) {
    next.kind = Some(PhaseKind::of_break(kind));
    next.timer = Timer::new(at, settings.config.break_ms(kind)).ok();
    next.planned_ms = None;
    next.focus_started_at = None;
    next.focus_ended_at = None;
    next.focus_elapsed_ms = None;
}

fn break_record(prev: &TimerState, ended_at: i64) -> Result<Record, CommandError> {
    let timer = timer_of(prev)?;
    let kind = prev
        .kind
        .filter(|k| *k != PhaseKind::Focus)
        .ok_or(CommandError::Rejected("unknown"))?;
    Ok(Record::Break {
        kind,
        started_at: timer.start(),
        ended_at,
        elapsed: timer.elapsed(ended_at),
        planned: timer.total(),
    })
}

/// The focus work that just finished, from the live timer (a focus phase that ended without going through reflect).
fn focus_record_from_timer(prev: &TimerState, ended_at: i64) -> Result<Record, CommandError> {
    let timer = timer_of(prev)?;
    Ok(Record::Focus {
        started_at: prev.focus_started_at.unwrap_or_else(|| timer.start()),
        ended_at,
        elapsed: timer.elapsed(ended_at),
        planned: prev.planned_ms.unwrap_or_else(|| timer.total()),
        intention: prev.intention.clone(),
        category: prev.category.clone(),
    })
}

/// The focus work held in `reflect`.
fn focus_record_from_reflect(prev: &TimerState) -> Result<Record, CommandError> {
    let (started_at, ended_at, elapsed) = prev
        .focus_started_at
        .zip(prev.focus_ended_at)
        .zip(prev.focus_elapsed_ms)
        .map(|((s, e), d)| (s, e, d))
        .ok_or(CommandError::Rejected("unknown"))?;
    Ok(Record::Focus {
        started_at,
        ended_at,
        elapsed,
        planned: prev.planned_ms.unwrap_or(elapsed),
        intention: prev.intention.clone(),
        category: prev.category.clone(),
    })
}

fn keep_break(next: &mut TimerState, record: &Record) {
    if matches!(
        record,
        Record::Break {
            kind: PhaseKind::LongBreak,
            ..
        }
    ) {
        next.rounds = 0;
    }
}

/// Work out the next state and what to record. `out` is the state machine's verdict for `event`.
fn build_next(
    prev: &TimerState,
    event: Event,
    out: Outcome,
    cmd: &Command,
    at: i64,
    settings: &Settings,
) -> Result<(TimerState, Vec<Record>), CommandError> {
    let mut next = TimerState {
        state: out.state,
        ..prev.clone()
    };
    let mut records = Vec::new();
    let start_focus = |next: &mut TimerState,
                       prev: &TimerState,
                       records: &mut Vec<Record>,
                       at: i64|
     -> Result<(), CommandError> {
        if out.effect == Some(Effect::SaveBreak) {
            let record = break_record(prev, at)?;
            keep_break(next, &record);
            records.push(record);
        }
        begin_focus(next, out.state, at, settings);
        Ok(())
    };

    match event {
        Event::InitFocus | Event::StartFocus => {
            let planned = match cmd.duration_ms {
                None => settings.config.focus_ms(),
                Some(d) if (MIN_PHASE_MS..=MAX_PHASE_MS).contains(&d) => d,
                Some(_) => return Err(CommandError::Invalid("bad_args")),
            };
            next.planned_ms = Some(planned);
            next.intention = clean(&cmd.intention, 200)?;
            next.category = clean(&cmd.category, 64)?;
            start_focus(&mut next, prev, &mut records, at)?;
        }
        Event::SkipBreathe => begin_session(&mut next, at),
        Event::TimerEnds => {
            let timer = timer_of(prev)?;
            if !timer.is_ended(at) || timer.is_paused() {
                return Err(CommandError::Rejected("not_ended"));
            }
            // A phase that auto-starts the next one starts it where the last one ended, not when we noticed.
            let end = timer.end_instant().min(at);
            match (prev.state, out.state) {
                (State::Breathe, _) => begin_session(&mut next, end),
                (State::Session, State::Rest) => {
                    records.push(focus_record_from_timer(prev, end)?);
                    next.rounds += 1;
                    let kind = settings.config.break_after(next.rounds);
                    begin_break(&mut next, kind, end, settings);
                }
                (State::Rest, State::Breathe | State::Session) => {
                    let record = break_record(prev, end)?;
                    keep_break(&mut next, &record);
                    records.push(record);
                    next.planned_ms = prev.planned_ms.or(Some(settings.config.focus_ms()));
                    begin_focus(&mut next, out.state, end, settings);
                }
                _ => {} // into overflow: the timer keeps counting
            }
        }
        Event::TogglePause => {
            let mut timer = timer_of(prev)?.clone();
            let result = if timer.is_paused() {
                timer.resume(at)
            } else {
                timer.pause(at)
            };
            result.map_err(|_| CommandError::Rejected("unknown"))?;
            next.timer = Some(timer);
        }
        Event::Extend { .. } => {
            let delta = cmd.delta_ms.ok_or(CommandError::Invalid("bad_args"))?;
            let mut timer = timer_of(prev)?.clone();
            timer.extend(delta, at);
            next.timer = Some(timer);
        }
        Event::Stop => {
            let timer = timer_of(prev)?;
            if out.state == State::Reflect {
                next.focus_ended_at = Some(at);
                next.focus_elapsed_ms = Some(timer.elapsed(at));
                next.kind = None;
                next.timer = None;
            } else {
                let record = break_record(prev, at)?;
                next = prev.cleared();
                keep_break(&mut next, &record);
                records.push(record);
            }
        }
        Event::Abandon => next = prev.cleared(),
        Event::TakeRest => {
            let explicit = match cmd.kind.as_deref() {
                Some("short") => Some(BreakKind::Short),
                Some("long") => Some(BreakKind::Long),
                None => None,
                Some(_) => return Err(CommandError::Invalid("bad_args")),
            };
            let from_reflect = out.effect == Some(Effect::SaveFocus);
            if from_reflect {
                records.push(focus_record_from_reflect(prev)?);
                next.rounds += 1;
            }
            // The cycle picks the break after a kept focus round; an unscheduled break is a short one.
            let kind = explicit.unwrap_or_else(|| {
                if from_reflect {
                    settings.config.break_after(next.rounds)
                } else {
                    BreakKind::Short
                }
            });
            begin_break(&mut next, kind, at, settings);
        }
        Event::Done => {
            records.push(focus_record_from_reflect(prev)?);
            next = prev.cleared();
            next.rounds += 1;
        }
        Event::SkipRest => {
            let record = break_record(prev, at)?;
            next = prev.cleared();
            keep_break(&mut next, &record);
            records.push(record);
            if out.state == State::Breathe || out.state == State::Session {
                next.state = out.state;
                next.planned_ms = prev.planned_ms.or(Some(settings.config.focus_ms()));
                next.intention = prev.intention.clone();
                next.category = prev.category.clone();
                begin_focus(&mut next, out.state, at, settings);
            }
        }
    }
    next.state = out.state;
    Ok((next, records))
}

fn record_event(record: &Record, ids: &mut IdGen, now_ms: i64) -> EventRecord {
    let id = ids.next(now_ms);
    let payload = match record {
        Record::Focus {
            started_at,
            ended_at,
            elapsed,
            planned,
            intention,
            category,
        } => json!({
            "kind": "focus",
            "started_at": started_at,
            "ended_at": ended_at,
            "duration_ms": elapsed,
            "planned_ms": planned,
            "intention": intention,
            "category": category,
        }),
        Record::Break {
            kind,
            started_at,
            ended_at,
            elapsed,
            planned,
        } => json!({
            "kind": kind.code(),
            "started_at": started_at,
            "ended_at": ended_at,
            "duration_ms": elapsed,
            "planned_ms": planned,
        }),
    };
    EventRecord {
        id: id.clone(),
        entity: Entity::Session,
        op: Op::Add,
        key: id,
        payload,
    }
}

/// Apply one command for `user`. The caller holds the database exclusively, so load-decide-write is atomic.
pub fn apply_command(
    db: &mut Db,
    user: &str,
    cmd: &Command,
    now_ms: i64,
    ids: &mut IdGen,
) -> Result<CommandResult, CommandError> {
    validate::check_id(&cmd.id, now_ms).map_err(CommandError::Invalid)?;
    // A retry of a command that already worked is answered as such, whatever state the timer is in now.
    if db.command_seen(user, &cmd.id)? {
        return Ok(CommandResult::Duplicate);
    }
    let at = cmd.at.unwrap_or(now_ms);
    if at < 0 {
        return Err(CommandError::Invalid("bad_time"));
    }
    if at > now_ms + MAX_TIME_SKEW_MS {
        return Err(CommandError::Invalid("time_in_future"));
    }

    let delta = cmd.delta_ms.unwrap_or(0);
    let step_ok = delta != 0
        && delta % EXTEND_STEP_MS == 0
        && delta.abs() <= MAX_EXTEND_STEPS * EXTEND_STEP_MS;
    let prev = TimerState::load(db, user)?;
    let settings = Settings::load(db, user)?;

    let event = if cmd.name == "extend" {
        if !step_ok {
            return Err(CommandError::Invalid("bad_args"));
        }
        let ended = prev
            .timer
            .as_ref()
            .map(|t| {
                let mut t = t.clone();
                t.extend(delta, at);
                t.is_ended(at)
            })
            .unwrap_or(false);
        Event::Extend { phase_ended: ended }
    } else {
        Event::from_code(&cmd.name, false).ok_or(CommandError::Invalid("unknown_command"))?
    };

    let out = transition(prev.state, event, settings.rules).map_err(reject)?;
    let (next, records) = build_next(&prev, event, out, cmd, at, &settings)?;

    let timer_event = EventRecord {
        id: ids.next(now_ms),
        entity: Entity::Timer,
        op: Op::Add,
        key: "current".to_string(),
        payload: Value::Object(next.to_json()),
    };
    let mut events = vec![timer_event];
    events.extend(records.iter().map(|r| record_event(r, ids, now_ms)));

    match db.apply_command(user, &cmd.id, &events)? {
        None => Ok(CommandResult::Duplicate),
        Some(applied) => {
            // The timer row must always take the new state; anything else means the log and this code disagree.
            if applied
                .first()
                .is_some_and(|a| !matches!(a, Applied::Added | Applied::Edited))
            {
                return Err(CommandError::Db(format!(
                    "timer event was not applied: {:?}",
                    applied.first()
                )));
            }
            Ok(CommandResult::Applied(next))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Role, User};
    use crate::settings;

    const MIN: i64 = 60_000;
    const START: i64 = 1_700_000_000_000;
    const ALICE: &str = "alice";

    struct Harness {
        db: Db,
        server_ids: IdGen,
        client_ids: IdGen,
        now: i64,
    }

    impl Harness {
        fn new() -> Self {
            let db = Db::open_in_memory().expect("db");
            for name in [ALICE, "bob"] {
                db.insert_user(&User {
                    id: name.into(),
                    username: name.into(),
                    password_hash: "h".into(),
                    role: Role::User,
                    disabled: false,
                    created_at: 0,
                })
                .expect("user");
            }
            Self {
                db,
                server_ids: IdGen::new("server"),
                client_ids: IdGen::new("client"),
                now: START,
            }
        }

        fn advance(&mut self, ms: i64) {
            self.now += ms;
        }

        fn setting(&mut self, name: &str, value: Value) {
            let id = self.client_ids.next(self.now);
            let ev = EventRecord {
                id,
                entity: Entity::Setting,
                op: Op::Add,
                key: name.into(),
                payload: json!({"value": value}),
            };
            self.db.apply_event(ALICE, &ev).expect("setting");
        }

        fn cmd(&mut self, name: &str) -> Command {
            Command {
                id: self.client_ids.next(self.now),
                name: name.into(),
                at: None,
                intention: None,
                category: None,
                duration_ms: None,
                delta_ms: None,
                kind: None,
            }
        }

        fn run_as(&mut self, user: &str, cmd: &Command) -> Result<CommandResult, CommandError> {
            apply_command(&mut self.db, user, cmd, self.now, &mut self.server_ids)
        }

        fn run(&mut self, name: &str) -> TimerState {
            let cmd = self.cmd(name);
            self.apply(&cmd)
        }

        fn apply(&mut self, cmd: &Command) -> TimerState {
            match self.run_as(ALICE, cmd) {
                Ok(CommandResult::Applied(s)) => s,
                other => panic!("{} failed: {other:?}", cmd.name),
            }
        }

        fn err(&mut self, cmd: &Command) -> CommandError {
            self.run_as(ALICE, cmd).expect_err("expected an error")
        }

        fn state(&self) -> TimerState {
            TimerState::load(&self.db, ALICE).expect("load")
        }

        /// Stored sessions as (kind, duration_ms), oldest first.
        fn sessions(&self) -> Vec<(String, i64)> {
            self.db
                .rows(ALICE, Entity::Session)
                .expect("rows")
                .into_iter()
                .map(|(_, r)| {
                    (
                        r.data["kind"].as_str().expect("kind").to_string(),
                        r.data["duration_ms"].as_i64().expect("duration"),
                    )
                })
                .collect()
        }
    }

    #[test]
    fn starting_focus_runs_the_users_focus_length() {
        let mut h = Harness::new();
        let mut cmd = h.cmd("init_focus");
        cmd.intention = Some("  write the report ".into());
        cmd.category = Some("work".into());
        let s = h.apply(&cmd);
        assert_eq!(s.state, State::Session);
        assert_eq!(s.kind, Some(PhaseKind::Focus));
        let t = s.timer.as_ref().expect("timer");
        assert_eq!((t.start(), t.total()), (START, 25 * MIN));
        assert_eq!(s.intention.as_deref(), Some("write the report"));
        assert_eq!(h.state(), s, "what was returned is what was stored");
    }

    #[test]
    fn a_full_cycle_records_focus_and_break_sessions() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(25 * MIN);
        assert_eq!(h.run("timer_ends").state, State::SessionEnd);
        h.advance(2 * MIN); // overflow
        let reflect = h.run("stop");
        assert_eq!(reflect.state, State::Reflect);
        assert_eq!(reflect.focus_elapsed_ms, Some(27 * MIN));
        assert!(
            h.sessions().is_empty(),
            "nothing is stored until reflect is left"
        );

        let rest = h.run("take_rest");
        assert_eq!(
            (rest.state, rest.kind, rest.rounds),
            (State::Rest, Some(PhaseKind::ShortBreak), 1)
        );
        assert_eq!(h.sessions(), [("focus".to_string(), 27 * MIN)]);

        h.advance(5 * MIN);
        assert_eq!(h.run("timer_ends").state, State::RestEnd);
        let next = h.run("init_focus");
        assert_eq!(next.state, State::Session);
        assert_eq!(
            h.sessions(),
            [
                ("focus".to_string(), 27 * MIN),
                ("short_break".to_string(), 5 * MIN)
            ]
        );
    }

    #[test]
    fn commands_that_do_not_fit_the_state_are_rejected() {
        let mut h = Harness::new();
        let done = h.cmd("done");
        assert_eq!(h.err(&done), CommandError::Rejected("unknown"));
        h.run("init_focus");
        let ends = h.cmd("timer_ends");
        assert_eq!(
            h.err(&ends),
            CommandError::Rejected("not_ended"),
            "the clock has not reached the end"
        );
        h.advance(24 * MIN + 59_000);
        let ends = h.cmd("timer_ends");
        assert_eq!(h.err(&ends), CommandError::Rejected("not_ended"));
        h.advance(1_000);
        let ends = h.cmd("timer_ends");
        assert!(matches!(
            h.run_as(ALICE, &ends),
            Ok(CommandResult::Applied(_))
        ));
    }

    #[test]
    fn a_rejected_command_can_be_retried_with_the_same_id() {
        let mut h = Harness::new();
        h.run("init_focus");
        let ends = h.cmd("timer_ends");
        assert_eq!(h.err(&ends), CommandError::Rejected("not_ended"));
        h.advance(25 * MIN);
        assert!(
            matches!(h.run_as(ALICE, &ends), Ok(CommandResult::Applied(_))),
            "the id was not used up"
        );
    }

    #[test]
    fn pause_freezes_the_clock_and_resume_continues_it() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(10 * MIN);
        let paused = h.run("toggle_pause");
        assert_eq!(paused.state, State::Paused(op_core::PausedFrom::Session));
        h.advance(10 * MIN);
        let resumed = h.run("toggle_pause");
        assert_eq!(resumed.state, State::Session);
        let t = resumed.timer.as_ref().expect("timer");
        assert_eq!(t.elapsed(h.now), 10 * MIN);
        h.advance(14 * MIN + 59_000);
        let ends = h.cmd("timer_ends");
        assert_eq!(h.err(&ends), CommandError::Rejected("not_ended"));
        h.advance(1_000);
        h.run("timer_ends");
    }

    #[test]
    fn extend_and_shrink_move_the_end_in_five_minute_steps() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(10 * MIN);
        let mut up = h.cmd("extend");
        up.delta_ms = Some(5 * MIN);
        assert_eq!(h.apply(&up).timer.expect("t").total(), 30 * MIN);
        let mut down = h.cmd("extend");
        down.delta_ms = Some(-10 * MIN);
        assert_eq!(h.apply(&down).timer.expect("t").total(), 20 * MIN);
        for bad in [
            None,
            Some(0),
            Some(7 * MIN),
            Some(65 * MIN),
            Some(-65 * MIN),
        ] {
            let mut c = h.cmd("extend");
            c.delta_ms = bad;
            assert_eq!(h.err(&c), CommandError::Invalid("bad_args"), "{bad:?}");
        }
    }

    #[test]
    fn extending_out_of_overflow_runs_the_session_again() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(30 * MIN);
        assert_eq!(h.run("timer_ends").state, State::SessionEnd);
        let mut more = h.cmd("extend");
        more.delta_ms = Some(5 * MIN);
        let s = h.apply(&more);
        assert_eq!(s.state, State::Session);
        assert_eq!(s.timer.expect("t").remaining(h.now), 5 * MIN);
    }

    #[test]
    fn shrinking_past_the_end_still_needs_timer_ends() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(20 * MIN);
        let mut down = h.cmd("extend");
        down.delta_ms = Some(-10 * MIN);
        let s = h.apply(&down);
        assert_eq!(s.state, State::Session, "the machine waits for timer_ends");
        assert!(s.timer.expect("t").is_ended(h.now));
        assert_eq!(h.run("timer_ends").state, State::SessionEnd);
    }

    #[test]
    fn the_breathing_step_comes_first_when_enabled() {
        let mut h = Harness::new();
        h.setting("breathe", json!(true));
        let s = h.run("init_focus");
        assert_eq!(s.state, State::Breathe);
        assert_eq!(
            s.timer.as_ref().expect("t").total(),
            settings::DEFAULT_BREATHE_MS
        );
        h.advance(settings::DEFAULT_BREATHE_MS + 4_000);
        let session = h.run("timer_ends");
        assert_eq!(session.state, State::Session);
        let t = session.timer.expect("t");
        assert_eq!(
            t.start(),
            START + settings::DEFAULT_BREATHE_MS,
            "the session starts where breathing ended"
        );
        assert_eq!(
            session.focus_started_at,
            Some(START + settings::DEFAULT_BREATHE_MS)
        );
    }

    #[test]
    fn skipping_the_breathing_step_starts_now() {
        let mut h = Harness::new();
        h.setting("breathe", json!(true));
        h.run("init_focus");
        h.advance(5_000);
        let s = h.run("skip_breathe");
        assert_eq!(s.state, State::Session);
        assert_eq!(s.timer.expect("t").start(), START + 5_000);
    }

    #[test]
    fn auto_start_break_begins_the_break_where_focus_ended() {
        let mut h = Harness::new();
        h.setting("auto_start_break", json!(true));
        h.run("init_focus");
        h.advance(40 * MIN); // the app was asleep for 15 minutes past the end
        let rest = h.run("timer_ends");
        assert_eq!((rest.state, rest.rounds), (State::Rest, 1));
        assert_eq!(
            rest.timer.expect("t").start(),
            START + 25 * MIN,
            "no drift from noticing late"
        );
        assert_eq!(h.sessions(), [("focus".to_string(), 25 * MIN)]);
    }

    #[test]
    fn auto_start_focus_chains_the_next_round_and_keeps_the_intention() {
        let mut h = Harness::new();
        h.setting("auto_start_break", json!(true));
        h.setting("auto_start_focus", json!(true));
        let mut start = h.cmd("init_focus");
        start.intention = Some("deep work".into());
        h.apply(&start);
        h.advance(25 * MIN);
        h.run("timer_ends"); // into the break
        h.advance(5 * MIN);
        let next = h.run("timer_ends"); // break over, next focus
        assert_eq!(next.state, State::Session);
        assert_eq!(next.intention.as_deref(), Some("deep work"));
        assert_eq!(next.timer.expect("t").start(), START + 30 * MIN);
        assert_eq!(h.sessions().len(), 2);
    }

    #[test]
    fn every_nth_round_earns_a_long_break_and_it_resets_the_count() {
        let mut h = Harness::new();
        h.setting("rounds_before_long", json!(2));
        let mut kinds = Vec::new();
        for _ in 0..2 {
            h.run("init_focus");
            h.advance(25 * MIN);
            h.run("timer_ends");
            h.run("stop");
            kinds.push(h.run("take_rest").kind);
            h.advance(15 * MIN); // long enough to end either kind of break
            h.run("timer_ends");
        }
        assert_eq!(
            kinds,
            [Some(PhaseKind::ShortBreak), Some(PhaseKind::LongBreak)]
        );
        assert_eq!(h.state().rounds, 2);
        h.run("init_focus"); // saves the long break
        assert_eq!(h.state().rounds, 0, "a long break starts a fresh cycle");
    }

    #[test]
    fn take_rest_can_choose_the_break_and_rejects_nonsense() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(25 * MIN);
        h.run("timer_ends");
        h.run("stop");
        let mut bad = h.cmd("take_rest");
        bad.kind = Some("huge".into());
        assert_eq!(h.err(&bad), CommandError::Invalid("bad_args"));
        let mut long = h.cmd("take_rest");
        long.kind = Some("long".into());
        assert_eq!(h.apply(&long).kind, Some(PhaseKind::LongBreak));
    }

    #[test]
    fn abandon_keeps_nothing_and_does_not_count_a_round() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(10 * MIN);
        let s = h.run("abandon");
        assert_eq!((s.state, s.rounds), (State::Idle, 0));
        assert!(h.sessions().is_empty());
        assert_eq!(h.state().timer, None);
    }

    #[test]
    fn done_keeps_the_focus_session_and_goes_idle() {
        let mut h = Harness::new();
        h.run("init_focus");
        h.advance(25 * MIN);
        h.run("timer_ends");
        h.run("stop");
        let s = h.run("done");
        assert_eq!((s.state, s.rounds), (State::Idle, 1));
        assert_eq!(h.sessions().len(), 1);
    }

    #[test]
    fn stopping_a_break_early_keeps_its_real_length() {
        let mut h = Harness::new();
        h.run("take_rest"); // an unscheduled break from idle
        h.advance(2 * MIN);
        let s = h.run("stop");
        assert_eq!(s.state, State::Idle);
        assert_eq!(h.sessions(), [("short_break".to_string(), 2 * MIN)]);
    }

    #[test]
    fn skipping_a_break_can_start_the_next_focus() {
        let mut h = Harness::new();
        h.setting("auto_start_focus", json!(true));
        h.run("take_rest");
        h.advance(MIN);
        let s = h.run("skip_rest");
        assert_eq!(s.state, State::Session);
        assert_eq!(h.sessions(), [("short_break".to_string(), MIN)]);
    }

    #[test]
    fn strict_mode_blocks_pausing_and_abandoning() {
        let mut h = Harness::new();
        h.setting("strict", json!(true));
        h.run("init_focus");
        for name in ["toggle_pause", "abandon"] {
            let c = h.cmd(name);
            assert_eq!(h.err(&c), CommandError::Rejected("strict"), "{name}");
        }
        assert_eq!(h.state().state, State::Session, "nothing changed");
    }

    #[test]
    fn a_repeated_command_id_changes_nothing() {
        let mut h = Harness::new();
        let cmd = h.cmd("init_focus");
        let first = h.apply(&cmd);
        h.advance(MIN);
        assert_eq!(h.run_as(ALICE, &cmd), Ok(CommandResult::Duplicate));
        assert_eq!(h.state(), first);
        assert_eq!(h.db.rows(ALICE, Entity::Session).expect("rows").len(), 0);
    }

    #[test]
    fn an_offline_replay_uses_the_time_the_user_acted() {
        let mut h = Harness::new();
        h.advance(30 * MIN); // the server hears about it half an hour later
        let mut start = h.cmd("init_focus");
        start.at = Some(START);
        let s = h.apply(&start);
        assert_eq!(s.timer.as_ref().expect("t").start(), START);
        assert!(
            s.timer.expect("t").is_ended(h.now),
            "25 minutes of the 30 have already passed"
        );
        let mut ends = h.cmd("timer_ends");
        ends.at = Some(START + 25 * MIN);
        assert_eq!(h.apply(&ends).state, State::SessionEnd);
    }

    #[test]
    fn bad_times_ids_and_names_are_refused() {
        let mut h = Harness::new();
        let mut future = h.cmd("init_focus");
        future.at = Some(START + 10 * MIN);
        assert_eq!(h.err(&future), CommandError::Invalid("time_in_future"));
        let mut negative = h.cmd("init_focus");
        negative.at = Some(-1);
        assert_eq!(h.err(&negative), CommandError::Invalid("bad_time"));
        let mut junk_id = h.cmd("init_focus");
        junk_id.id = "nope".into();
        assert_eq!(h.err(&junk_id), CommandError::Invalid("bad_id"));
        let mut far = h.cmd("init_focus");
        far.id = IdGen::new("client").next(START + 3 * 24 * 60 * MIN);
        assert_eq!(h.err(&far), CommandError::Invalid("id_in_future"));
        let unknown = h.cmd("teleport");
        assert_eq!(h.err(&unknown), CommandError::Invalid("unknown_command"));
        assert_eq!(h.state(), TimerState::idle(), "nothing was written");
    }

    #[test]
    fn arguments_are_validated() {
        let mut h = Harness::new();
        for (duration, intention) in [
            (Some(5), None),
            (Some(MAX_PHASE_MS + 1), None),
            (None, Some("x".repeat(201))),
        ] {
            let mut c = h.cmd("init_focus");
            c.duration_ms = duration;
            c.intention = intention;
            assert_eq!(h.err(&c), CommandError::Invalid("bad_args"));
        }
        let mut custom = h.cmd("init_focus");
        custom.duration_ms = Some(50 * MIN);
        assert_eq!(h.apply(&custom).timer.expect("t").total(), 50 * MIN);
    }

    #[test]
    fn users_have_independent_timers() {
        let mut h = Harness::new();
        h.run("init_focus");
        let cmd = h.cmd("init_focus");
        assert!(
            matches!(h.run_as("bob", &cmd), Ok(CommandResult::Applied(_))),
            "Bob is idle, so he can start"
        );
        h.advance(25 * MIN);
        let bobs_end = h.cmd("timer_ends");
        h.run_as("bob", &bobs_end).expect("bob ends");
        assert_eq!(h.state().state, State::Session, "Alice is unaffected");
        assert_eq!(
            TimerState::load(&h.db, "bob").expect("bob").state,
            State::SessionEnd
        );
    }

    #[test]
    fn the_timer_row_always_takes_each_new_state() {
        // apply_command fails if the stored timer event was Stale; many quick commands prove it never is.
        let mut h = Harness::new();
        h.run("init_focus");
        for _ in 0..30 {
            h.run("toggle_pause");
        }
        assert_eq!(h.state().state, State::Session);
    }
}
