//! Runs a user's timer on the server: checks a command, asks the shared planner (`op_core::plan`) what it does
//! to the stored state, and writes the result.
//!
//! Every timer event is written with a server-made id, so the timer row always moves forward no matter how old
//! the client's own timestamp is. A command's own id is only an idempotency key.

use op_core::{Action, Applied, Entity, EventRecord, IdGen, Op, PlanError, Record, plan};
use serde::Deserialize;
use serde_json::Value;

use crate::db::{Db, DbError};
use crate::settings::Settings;
use crate::validate;

pub use op_core::{PhaseKind, TimerState};

/// A command may claim a time slightly ahead of the server clock (clock skew), but not more than this.
const MAX_TIME_SKEW_MS: i64 = 60_000;

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
    /// Notes and mood for the focus session this command saves.
    pub notes: Option<String>,
    pub mood: Option<String>,
}

impl Command {
    fn action(&self) -> Action {
        Action {
            name: self.name.clone(),
            intention: self.intention.clone(),
            category: self.category.clone(),
            duration_ms: self.duration_ms,
            delta_ms: self.delta_ms,
            kind: self.kind.clone(),
            notes: self.notes.clone(),
            mood: self.mood.clone(),
        }
    }
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

impl From<PlanError> for CommandError {
    fn from(e: PlanError) -> Self {
        match e {
            PlanError::Invalid(code) => CommandError::Invalid(code),
            PlanError::Rejected(code) => CommandError::Rejected(code),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CommandResult {
    Applied(TimerState),
    /// This command id was processed before; nothing changed.
    Duplicate,
}

/// The user's stored timer, or an idle one if they have none yet.
pub fn load_state(db: &Db, user: &str) -> Result<TimerState, DbError> {
    Ok(db
        .current_timer(user)?
        .map_or_else(TimerState::idle, |data| TimerState::from_data(&data)))
}

fn record_event(record: &Record, ids: &mut IdGen, now_ms: i64) -> EventRecord {
    let id = ids.next(now_ms);
    EventRecord {
        id: id.clone(),
        entity: Entity::Session,
        op: Op::Add,
        key: id,
        payload: record.to_json(),
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

    let prev = load_state(db, user)?;
    let settings = Settings::load(db, user)?;
    let planned = plan(&prev, &cmd.action(), at, &settings.plan_settings())?;

    let timer_event = EventRecord {
        id: ids.next(now_ms),
        entity: Entity::Timer,
        op: Op::Add,
        key: "current".to_string(),
        payload: Value::Object(planned.next.to_json()),
    };
    let mut events = vec![timer_event];
    events.extend(planned.records.iter().map(|r| record_event(r, ids, now_ms)));

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
            Ok(CommandResult::Applied(planned.next))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Role, User};
    use crate::settings;
    use op_core::State;
    use op_core::config::MAX_PHASE_MS;
    use serde_json::json;

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
                notes: None,
                mood: None,
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
            load_state(&self.db, ALICE).expect("load")
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
    fn editing_the_running_session_changes_what_is_recorded() {
        let mut h = Harness::new();
        let mut start = h.cmd("init_focus");
        start.intention = Some("draft".into());
        h.apply(&start);
        h.advance(10 * MIN);
        let mut edit = h.cmd("edit_session");
        edit.intention = Some("final report".into());
        edit.category = Some("work".into());
        let s = h.apply(&edit);
        assert_eq!(s.state, State::Session);
        assert_eq!(s.timer.as_ref().map(|t| t.total()), Some(25 * MIN));
        h.advance(15 * MIN);
        h.run("timer_ends");
        h.run("stop");
        h.run("done");
        let rows = h.db.rows(ALICE, Entity::Session).expect("rows");
        let (_, row) = rows.first().expect("one session");
        assert_eq!(row.data["intention"], "final report");
        assert_eq!(row.data["category"], "work");
    }

    #[test]
    fn a_break_can_be_started_with_its_own_length() {
        let mut h = Harness::new();
        let mut rest = h.cmd("take_rest");
        rest.duration_ms = Some(12 * MIN);
        let s = h.apply(&rest);
        assert_eq!(s.state, State::Rest);
        assert_eq!(s.timer.as_ref().map(|t| t.total()), Some(12 * MIN));
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
            load_state(&h.db, "bob").expect("bob").state,
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
