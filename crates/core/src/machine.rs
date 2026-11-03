//! The phase state machine as one table (see PLAN.md, section 2). Unknown `(state, event)` pairs are
//! rejected, never ignored: the caller logs the rejection.
//!
//! The machine is pure. It does not own a timer; the caller applies the timer change first and reports
//! what happened (`Extend { phase_ended }`) or fires `TimerEnds` when the timer says the phase is over.
//! It also does not write anything: it names what must be saved (`Effect`) when a transition happens.

/// Where a paused phase returns to on resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PausedFrom {
    Session,
    SessionEnd,
    Rest,
    RestEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Breathe,
    Session,
    /// Focus past its planned end (overflow).
    SessionEnd,
    Paused(PausedFrom),
    Reflect,
    Rest,
    /// Break past its planned end (overflow).
    RestEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    InitFocus,
    SkipBreathe,
    StartFocus,
    TimerEnds,
    TogglePause,
    /// The caller already applied the extend or shrink to the timer. `phase_ended` is the timer's
    /// answer afterwards: false lets an overflow phase run again.
    Extend {
        phase_ended: bool,
    },
    /// End the phase early and keep it.
    Stop,
    /// End a focus phase and keep nothing.
    Abandon,
    TakeRest,
    Done,
    SkipRest,
}

/// Settings that change the table. Strict mode blocks pausing, abandoning and cutting a break short.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rules {
    pub breathe: bool,
    pub auto_start_break: bool,
    pub auto_start_focus: bool,
    pub strict: bool,
}

/// Something the caller must persist because of the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    SaveFocus,
    SaveBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub state: State,
    pub effect: Option<Effect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// No row in the table for this pair.
    Unknown,
    /// The row exists but strict mode blocks it.
    Strict,
}

impl Rejected {
    pub fn code(self) -> &'static str {
        match self {
            Rejected::Unknown => "unknown",
            Rejected::Strict => "strict",
        }
    }
}

impl PausedFrom {
    fn state(self) -> State {
        match self {
            PausedFrom::Session => State::Session,
            PausedFrom::SessionEnd => State::SessionEnd,
            PausedFrom::Rest => State::Rest,
            PausedFrom::RestEnd => State::RestEnd,
        }
    }
}

impl State {
    /// Stable name used in storage and in the shared vectors.
    pub fn code(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Breathe => "breathe",
            State::Session => "session",
            State::SessionEnd => "session_end",
            State::Paused(PausedFrom::Session) => "paused_session",
            State::Paused(PausedFrom::SessionEnd) => "paused_session_end",
            State::Paused(PausedFrom::Rest) => "paused_rest",
            State::Paused(PausedFrom::RestEnd) => "paused_rest_end",
            State::Reflect => "reflect",
            State::Rest => "rest",
            State::RestEnd => "rest_end",
        }
    }

    pub fn from_code(code: &str) -> Option<State> {
        Some(match code {
            "idle" => State::Idle,
            "breathe" => State::Breathe,
            "session" => State::Session,
            "session_end" => State::SessionEnd,
            "paused_session" => State::Paused(PausedFrom::Session),
            "paused_session_end" => State::Paused(PausedFrom::SessionEnd),
            "paused_rest" => State::Paused(PausedFrom::Rest),
            "paused_rest_end" => State::Paused(PausedFrom::RestEnd),
            "reflect" => State::Reflect,
            "rest" => State::Rest,
            "rest_end" => State::RestEnd,
            _ => return None,
        })
    }
}

impl Event {
    pub fn code(self) -> &'static str {
        match self {
            Event::InitFocus => "init_focus",
            Event::SkipBreathe => "skip_breathe",
            Event::StartFocus => "start_focus",
            Event::TimerEnds => "timer_ends",
            Event::TogglePause => "toggle_pause",
            Event::Extend { .. } => "extend",
            Event::Stop => "stop",
            Event::Abandon => "abandon",
            Event::TakeRest => "take_rest",
            Event::Done => "done",
            Event::SkipRest => "skip_rest",
        }
    }

    /// `phase_ended` is only read for `extend`.
    pub fn from_code(code: &str, phase_ended: bool) -> Option<Event> {
        Some(match code {
            "init_focus" => Event::InitFocus,
            "skip_breathe" => Event::SkipBreathe,
            "start_focus" => Event::StartFocus,
            "timer_ends" => Event::TimerEnds,
            "toggle_pause" => Event::TogglePause,
            "extend" => Event::Extend { phase_ended },
            "stop" => Event::Stop,
            "abandon" => Event::Abandon,
            "take_rest" => Event::TakeRest,
            "done" => Event::Done,
            "skip_rest" => Event::SkipRest,
            _ => return None,
        })
    }
}

fn ok(state: State, effect: Option<Effect>) -> Result<Outcome, Rejected> {
    Ok(Outcome { state, effect })
}

/// First focus state: the breathing step when it is on, otherwise straight into the session.
fn focus_start(rules: Rules) -> State {
    if rules.breathe {
        State::Breathe
    } else {
        State::Session
    }
}

pub fn transition(state: State, event: Event, rules: Rules) -> Result<Outcome, Rejected> {
    use Effect::{SaveBreak, SaveFocus};
    use State::{Breathe, Idle, Paused, Reflect, Rest, RestEnd, Session, SessionEnd};

    match (state, event) {
        // Start a focus round.
        (Idle, Event::InitFocus) => ok(focus_start(rules), None),
        (RestEnd, Event::InitFocus) => ok(focus_start(rules), Some(SaveBreak)),
        (Idle, Event::StartFocus) => ok(Session, None),
        (RestEnd, Event::StartFocus) => ok(Session, Some(SaveBreak)),
        (Breathe, Event::SkipBreathe) => ok(Session, None),

        // The timer says the phase is over.
        (Breathe, Event::TimerEnds) => ok(Session, None),
        (Session, Event::TimerEnds) if rules.auto_start_break => ok(Rest, Some(SaveFocus)),
        (Session, Event::TimerEnds) => ok(SessionEnd, None),
        (Rest, Event::TimerEnds) if rules.auto_start_focus => {
            ok(focus_start(rules), Some(SaveBreak))
        }
        (Rest, Event::TimerEnds) => ok(RestEnd, None),

        // Pause and resume. Strict mode blocks pausing only; a paused phase can always resume.
        (Session | SessionEnd | Rest | RestEnd, Event::TogglePause) if rules.strict => {
            Err(Rejected::Strict)
        }
        (Session, Event::TogglePause) => ok(Paused(PausedFrom::Session), None),
        (SessionEnd, Event::TogglePause) => ok(Paused(PausedFrom::SessionEnd), None),
        (Rest, Event::TogglePause) => ok(Paused(PausedFrom::Rest), None),
        (RestEnd, Event::TogglePause) => ok(Paused(PausedFrom::RestEnd), None),
        (Paused(from), Event::TogglePause) => ok(from.state(), None),

        // Extend or shrink. Only an overflow phase changes state, and only when the timer is running again.
        (Session | Rest | Paused(PausedFrom::Session | PausedFrom::Rest), Event::Extend { .. }) => {
            ok(state, None)
        }
        (SessionEnd, Event::Extend { phase_ended }) => {
            ok(if phase_ended { SessionEnd } else { Session }, None)
        }
        (RestEnd, Event::Extend { phase_ended }) => {
            ok(if phase_ended { RestEnd } else { Rest }, None)
        }
        (Paused(PausedFrom::SessionEnd), Event::Extend { phase_ended }) => ok(
            if phase_ended {
                state
            } else {
                Paused(PausedFrom::Session)
            },
            None,
        ),
        (Paused(PausedFrom::RestEnd), Event::Extend { phase_ended }) => ok(
            if phase_ended {
                state
            } else {
                Paused(PausedFrom::Rest)
            },
            None,
        ),

        // End early and keep it. A focus phase goes to reflect (saved when it leaves reflect); a break
        // is saved now. Strict mode blocks cutting a running break short but not one that is already over.
        (
            Session | SessionEnd | Paused(PausedFrom::Session | PausedFrom::SessionEnd),
            Event::Stop,
        ) => ok(Reflect, None),
        (Rest | Paused(PausedFrom::Rest), Event::Stop | Event::SkipRest) if rules.strict => {
            Err(Rejected::Strict)
        }
        (Rest | RestEnd | Paused(PausedFrom::Rest | PausedFrom::RestEnd), Event::Stop) => {
            ok(Idle, Some(SaveBreak))
        }
        (Rest | RestEnd | Paused(PausedFrom::Rest | PausedFrom::RestEnd), Event::SkipRest) => {
            let next = if rules.auto_start_focus {
                focus_start(rules)
            } else {
                Idle
            };
            ok(next, Some(SaveBreak))
        }

        // Abandon a focus phase: nothing is saved. Strict mode blocks it once the session has started.
        (Breathe, Event::Abandon) => ok(Idle, None),
        (
            Session | SessionEnd | Paused(PausedFrom::Session | PausedFrom::SessionEnd),
            Event::Abandon,
        ) if rules.strict => Err(Rejected::Strict),
        (
            Session | SessionEnd | Paused(PausedFrom::Session | PausedFrom::SessionEnd),
            Event::Abandon,
        ) => ok(Idle, None),

        // Leave reflect.
        (Reflect, Event::TakeRest) => ok(Rest, Some(SaveFocus)),
        (Idle, Event::TakeRest) => ok(Rest, None),
        (Reflect, Event::Done) => ok(Idle, Some(SaveFocus)),

        _ => Err(Rejected::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(state: State, event: Event, rules: Rules) -> State {
        transition(state, event, rules).expect("allowed").state
    }

    #[test]
    fn full_cycle_walk() {
        let r = Rules::default();
        let s = step(State::Idle, Event::InitFocus, r);
        assert_eq!(s, State::Session);
        let s = step(s, Event::TimerEnds, r);
        assert_eq!(s, State::SessionEnd);
        let s = step(s, Event::Stop, r);
        assert_eq!(s, State::Reflect);
        let out = transition(s, Event::TakeRest, r).expect("allowed");
        assert_eq!(
            (out.state, out.effect),
            (State::Rest, Some(Effect::SaveFocus))
        );
        let s = step(out.state, Event::TimerEnds, r);
        assert_eq!(s, State::RestEnd);
        let out = transition(s, Event::InitFocus, r).expect("allowed");
        assert_eq!(
            (out.state, out.effect),
            (State::Session, Some(Effect::SaveBreak))
        );
    }

    #[test]
    fn pause_remembers_where_it_came_from() {
        let r = Rules::default();
        for from in [
            State::Session,
            State::SessionEnd,
            State::Rest,
            State::RestEnd,
        ] {
            let paused = step(from, Event::TogglePause, r);
            assert!(matches!(paused, State::Paused(_)));
            assert_eq!(step(paused, Event::TogglePause, r), from);
        }
    }

    #[test]
    fn strict_mode_can_still_resume() {
        let strict = Rules {
            strict: true,
            ..Rules::default()
        };
        let paused = State::Paused(PausedFrom::Session);
        assert_eq!(step(paused, Event::TogglePause, strict), State::Session);
        assert_eq!(
            transition(State::Session, Event::TogglePause, strict),
            Err(Rejected::Strict)
        );
    }

    #[test]
    fn auto_start_skips_reflect_but_still_saves_the_focus_session() {
        let r = Rules {
            auto_start_break: true,
            ..Rules::default()
        };
        let out = transition(State::Session, Event::TimerEnds, r).expect("allowed");
        assert_eq!(
            (out.state, out.effect),
            (State::Rest, Some(Effect::SaveFocus))
        );
    }

    #[test]
    fn extend_from_overflow_runs_again_only_when_the_timer_does() {
        let r = Rules::default();
        assert_eq!(
            step(State::SessionEnd, Event::Extend { phase_ended: false }, r),
            State::Session
        );
        assert_eq!(
            step(State::SessionEnd, Event::Extend { phase_ended: true }, r),
            State::SessionEnd
        );
        assert_eq!(
            step(
                State::Paused(PausedFrom::RestEnd),
                Event::Extend { phase_ended: false },
                r
            ),
            State::Paused(PausedFrom::Rest)
        );
    }

    #[test]
    fn state_and_event_codes_round_trip() {
        let states = [
            State::Idle,
            State::Breathe,
            State::Session,
            State::SessionEnd,
            State::Paused(PausedFrom::Session),
            State::Paused(PausedFrom::SessionEnd),
            State::Paused(PausedFrom::Rest),
            State::Paused(PausedFrom::RestEnd),
            State::Reflect,
            State::Rest,
            State::RestEnd,
        ];
        for s in states {
            assert_eq!(State::from_code(s.code()), Some(s));
        }
        assert_eq!(State::from_code("nope"), None);
        assert_eq!(
            Event::from_code("extend", true),
            Some(Event::Extend { phase_ended: true })
        );
        assert_eq!(Event::from_code("nope", false), None);
    }
}
