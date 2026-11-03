//! The timer's decisions, with no I/O: given the stored timer state, one action (start, pause, stop, ...), the
//! time it happened and the user's settings, work out the next timer state and the sessions that finished.
//!
//! The server runs this on stored state; the browser runs the TypeScript port of it while offline. Both pass the
//! same vectors (`test-vectors/planner.json`), so a command queued offline lands on the same result.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::config::{MAX_PHASE_MS, MIN_PHASE_MS};
use crate::{
    BreakKind, Config, EXTEND_STEP_MS, Effect, Event, Outcome, Rejected, Rules, State, Timer,
    transition,
};

/// Extend and shrink come in 5-minute steps, at most an hour at a time.
const MAX_EXTEND_STEPS: i64 = 12;

/// What the user asked for. The server wraps this with an id and a time; the arguments are only read by the
/// actions they belong to.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Action {
    pub name: String,
    pub intention: Option<String>,
    pub category: Option<String>,
    /// Focus length for `init_focus` / `start_focus`, or break length for `take_rest`; the user's setting when omitted.
    pub duration_ms: Option<i64>,
    /// Extend (positive) or shrink (negative) by whole 5-minute steps.
    pub delta_ms: Option<i64>,
    /// For `take_rest`: `short` or `long`, instead of the cycle's choice.
    pub kind: Option<String>,
    /// Notes for the focus session this action saves (`take_rest`, `done`, or `timer_ends` into an auto break).
    pub notes: Option<String>,
    /// How the session went: `focused`, `neutral` or `distracted`. Read in the same places as `notes`.
    pub mood: Option<String>,
}

/// The settings the timer runs on.
#[derive(Debug, Clone, Copy)]
pub struct PlanSettings {
    pub config: Config,
    pub rules: Rules,
    pub breathe_ms: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    /// The action itself is malformed (stable code for the client).
    Invalid(&'static str),
    /// The action is well formed but not allowed right now.
    Rejected(&'static str),
}

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

    /// Back to idle, keeping only the round count.
    fn cleared(&self) -> Self {
        Self {
            rounds: self.rounds,
            ..Self::idle()
        }
    }
}

/// A finished piece of work to store as a session row.
pub enum Record {
    Focus {
        started_at: i64,
        ended_at: i64,
        elapsed: i64,
        planned: i64,
        intention: Option<String>,
        category: Option<String>,
        notes: Option<String>,
        mood: Option<String>,
    },
    Break {
        kind: PhaseKind,
        started_at: i64,
        ended_at: i64,
        elapsed: i64,
        planned: i64,
    },
}

impl Record {
    /// The row to store for this record (a `session` entity payload).
    pub fn to_json(&self) -> Value {
        match self {
            Record::Focus {
                started_at,
                ended_at,
                elapsed,
                planned,
                intention,
                category,
                notes,
                mood,
            } => json!({
                "kind": "focus",
                "started_at": started_at,
                "ended_at": ended_at,
                "duration_ms": elapsed,
                "planned_ms": planned,
                "intention": intention,
                "category": category,
                "notes": notes,
                "mood": mood,
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
        }
    }
}

fn clean(text: &Option<String>, max_chars: usize) -> Result<Option<String>, PlanError> {
    let Some(raw) = text else { return Ok(None) };
    let t = raw.trim();
    if t.chars().count() > max_chars || t.chars().any(|c| c.is_control() && c != '\n') {
        return Err(PlanError::Invalid("bad_args"));
    }
    Ok(if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    })
}

fn reject(r: Rejected) -> PlanError {
    PlanError::Rejected(r.code())
}

fn timer_of(state: &TimerState) -> Result<&Timer, PlanError> {
    state.timer.as_ref().ok_or(PlanError::Rejected("unknown"))
}

/// Begin the breathing step, or the focus session itself, at time `at`.
fn begin_focus(next: &mut TimerState, new_state: State, at: i64, settings: &PlanSettings) {
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

fn begin_break(next: &mut TimerState, kind: BreakKind, at: i64, settings: &PlanSettings) {
    next.kind = Some(PhaseKind::of_break(kind));
    next.timer = Timer::new(at, settings.config.break_ms(kind)).ok();
    next.planned_ms = None;
    next.focus_started_at = None;
    next.focus_ended_at = None;
    next.focus_elapsed_ms = None;
}

fn break_record(prev: &TimerState, ended_at: i64) -> Result<Record, PlanError> {
    let timer = timer_of(prev)?;
    let kind = prev
        .kind
        .filter(|k| *k != PhaseKind::Focus)
        .ok_or(PlanError::Rejected("unknown"))?;
    Ok(Record::Break {
        kind,
        started_at: timer.start(),
        ended_at,
        elapsed: timer.elapsed(ended_at),
        planned: timer.total(),
    })
}

/// The focus work that just finished, from the live timer (a focus phase that ended without going through reflect).
fn focus_record_from_timer(
    prev: &TimerState,
    ended_at: i64,
    extras: Extras,
) -> Result<Record, PlanError> {
    let timer = timer_of(prev)?;
    Ok(Record::Focus {
        started_at: prev.focus_started_at.unwrap_or_else(|| timer.start()),
        ended_at,
        elapsed: timer.elapsed(ended_at),
        planned: prev.planned_ms.unwrap_or_else(|| timer.total()),
        intention: prev.intention.clone(),
        category: prev.category.clone(),
        notes: extras.notes,
        mood: extras.mood,
    })
}

/// The focus work held in `reflect`.
fn focus_record_from_reflect(prev: &TimerState, extras: Extras) -> Result<Record, PlanError> {
    let (started_at, ended_at, elapsed) = prev
        .focus_started_at
        .zip(prev.focus_ended_at)
        .zip(prev.focus_elapsed_ms)
        .map(|((s, e), d)| (s, e, d))
        .ok_or(PlanError::Rejected("unknown"))?;
    Ok(Record::Focus {
        started_at,
        ended_at,
        elapsed,
        planned: prev.planned_ms.unwrap_or(elapsed),
        intention: prev.intention.clone(),
        category: prev.category.clone(),
        notes: extras.notes,
        mood: extras.mood,
    })
}

/// Notes and mood that go with a focus session when it is saved.
struct Extras {
    notes: Option<String>,
    mood: Option<String>,
}

const MAX_NOTES_CHARS: usize = 4000;

fn extras_of(action: &Action) -> Result<Extras, PlanError> {
    let mood = match action.mood.as_deref() {
        None => None,
        Some(m @ ("focused" | "neutral" | "distracted")) => Some(m.to_string()),
        Some(_) => return Err(PlanError::Invalid("bad_args")),
    };
    Ok(Extras {
        notes: clean(&action.notes, MAX_NOTES_CHARS)?,
        mood,
    })
}

/// A focus session this short with no intention and no notes is an accidental start, not work (C-06).
const JUNK_MS: i64 = 30_000;

/// Save a finished focus session: it goes into `records`, unless it was accidental. It counts as a round only if it
/// ran its full planned length: a session ended early is kept in the log but does not move the cycle on.
fn keep_focus(next: &mut TimerState, records: &mut Vec<Record>, record: Record) {
    let accidental = matches!(
        &record,
        Record::Focus { elapsed, intention: None, notes: None, .. } if *elapsed <= JUNK_MS
    );
    if accidental {
        return;
    }
    let full = matches!(&record, Record::Focus { elapsed, planned, .. } if elapsed >= planned);
    records.push(record);
    if full {
        next.rounds += 1;
    }
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
    cmd: &Action,
    at: i64,
    settings: &PlanSettings,
) -> Result<(TimerState, Vec<Record>), PlanError> {
    let mut next = TimerState {
        state: out.state,
        ..prev.clone()
    };
    let mut records = Vec::new();
    let start_focus = |next: &mut TimerState,
                       prev: &TimerState,
                       records: &mut Vec<Record>,
                       at: i64|
     -> Result<(), PlanError> {
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
                Some(_) => return Err(PlanError::Invalid("bad_args")),
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
                return Err(PlanError::Rejected("not_ended"));
            }
            // A phase that auto-starts the next one starts it where the last one ended, not when we noticed.
            let end = timer.end_instant().min(at);
            match (prev.state, out.state) {
                (State::Breathe, _) => begin_session(&mut next, end),
                (State::Session, State::Rest) => {
                    keep_focus(
                        &mut next,
                        &mut records,
                        focus_record_from_timer(prev, end, extras_of(cmd)?)?,
                    );
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
            result.map_err(|_| PlanError::Rejected("unknown"))?;
            next.timer = Some(timer);
        }
        Event::Extend { .. } => {
            let delta = cmd.delta_ms.ok_or(PlanError::Invalid("bad_args"))?;
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
                Some(_) => return Err(PlanError::Invalid("bad_args")),
            };
            let from_reflect = out.effect == Some(Effect::SaveFocus);
            if from_reflect {
                keep_focus(
                    &mut next,
                    &mut records,
                    focus_record_from_reflect(prev, extras_of(cmd)?)?,
                );
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
            if let Some(length) = cmd.duration_ms {
                if !(MIN_PHASE_MS..=MAX_PHASE_MS).contains(&length) {
                    return Err(PlanError::Invalid("bad_args"));
                }
                next.timer = Timer::new(at, length).ok();
            }
        }
        Event::Done => {
            next = prev.cleared();
            keep_focus(
                &mut next,
                &mut records,
                focus_record_from_reflect(prev, extras_of(cmd)?)?,
            );
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

pub struct Plan {
    pub next: TimerState,
    pub records: Vec<Record>,
}

/// Change what the focus session in progress is about (its intention and category) without touching its clock.
/// Allowed while a focus runs (also in overtime or paused) and on the reflect screen; both fields are replaced, and
/// an empty one clears it.
fn edit_session(prev: &TimerState, action: &Action) -> Result<Plan, PlanError> {
    let running_focus = prev.kind == Some(PhaseKind::Focus) && prev.timer.is_some();
    if !running_focus && prev.state != State::Reflect {
        return Err(PlanError::Rejected("unknown"));
    }
    let mut next = prev.clone();
    next.intention = clean(&action.intention, 200)?;
    next.category = clean(&action.category, 64)?;
    Ok(Plan {
        next,
        records: Vec::new(),
    })
}

/// Decide what `action`, taken at time `at`, does to the timer.
pub fn plan(
    prev: &TimerState,
    action: &Action,
    at: i64,
    settings: &PlanSettings,
) -> Result<Plan, PlanError> {
    if action.name == "edit_session" {
        return edit_session(prev, action);
    }
    let event = if action.name == "extend" {
        let delta = action.delta_ms.unwrap_or(0);
        let step_ok = delta != 0
            && delta % EXTEND_STEP_MS == 0
            && delta.abs() <= MAX_EXTEND_STEPS * EXTEND_STEP_MS;
        if !step_ok {
            return Err(PlanError::Invalid("bad_args"));
        }
        let ended = prev.timer.as_ref().is_some_and(|t| {
            let mut t = t.clone();
            t.extend(delta, at);
            t.is_ended(at)
        });
        Event::Extend { phase_ended: ended }
    } else {
        Event::from_code(&action.name, false).ok_or(PlanError::Invalid("unknown_command"))?
    };

    let out = transition(prev.state, event, settings.rules).map_err(reject)?;
    let (next, records) = build_next(prev, event, out, action, at, settings)?;
    Ok(Plan { next, records })
}
