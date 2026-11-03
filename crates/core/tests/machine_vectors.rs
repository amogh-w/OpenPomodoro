//! Runs the shared state machine vectors (test-vectors/machine.json). The TypeScript port runs the same file.

use std::collections::HashSet;

use op_core::{Effect, Event, Rejected, Rules, State, transition};
use serde::Deserialize;

#[derive(Deserialize)]
struct Suite {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    state: String,
    event: String,
    phase_ended: Option<bool>,
    rules: RuleFlags,
    expect: Expect,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RuleFlags {
    breathe: bool,
    auto_start_break: bool,
    auto_start_focus: bool,
    strict: bool,
}

#[derive(Deserialize)]
struct Expect {
    state: Option<String>,
    effect: Option<String>,
    rejected: Option<String>,
}

const STATES: [&str; 11] = [
    "idle",
    "breathe",
    "session",
    "session_end",
    "paused_session",
    "paused_session_end",
    "paused_rest",
    "paused_rest_end",
    "reflect",
    "rest",
    "rest_end",
];

const EVENTS: [&str; 11] = [
    "init_focus",
    "skip_breathe",
    "start_focus",
    "timer_ends",
    "toggle_pause",
    "extend",
    "stop",
    "abandon",
    "take_rest",
    "done",
    "skip_rest",
];

fn effect_code(effect: Option<Effect>) -> Option<&'static str> {
    effect.map(|e| match e {
        Effect::SaveFocus => "save_focus",
        Effect::SaveBreak => "save_break",
    })
}

fn load() -> Suite {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/machine.json"
    );
    let text = std::fs::read_to_string(path).expect("read vectors");
    serde_json::from_str(&text).expect("parse vectors")
}

#[test]
fn shared_machine_vectors() {
    for (i, c) in load().cases.iter().enumerate() {
        let state = State::from_code(&c.state).expect("known state");
        let event =
            Event::from_code(&c.event, c.phase_ended.unwrap_or(false)).expect("known event");
        let rules = Rules {
            breathe: c.rules.breathe,
            auto_start_break: c.rules.auto_start_break,
            auto_start_focus: c.rules.auto_start_focus,
            strict: c.rules.strict,
        };
        let ctx = format!(
            "case {i}: {} + {} (ended {:?})",
            c.state, c.event, c.phase_ended
        );
        match transition(state, event, rules) {
            Ok(out) => {
                assert_eq!(c.expect.rejected, None, "{ctx}: expected a rejection");
                assert_eq!(
                    Some(out.state.code()),
                    c.expect.state.as_deref(),
                    "{ctx}: state"
                );
                assert_eq!(
                    effect_code(out.effect),
                    c.expect.effect.as_deref(),
                    "{ctx}: effect"
                );
            }
            Err(rejected) => {
                assert_eq!(
                    Some(rejected.code()),
                    c.expect.rejected.as_deref(),
                    "{ctx}: rejection"
                );
            }
        }
    }
}

/// Every state is tried with every event, so a new state or event cannot slip in without vectors.
#[test]
fn vectors_cover_every_pair() {
    let seen: HashSet<(String, String)> = load()
        .cases
        .into_iter()
        .map(|c| (c.state, c.event))
        .collect();
    for s in STATES {
        for e in EVENTS {
            assert!(
                seen.contains(&(s.to_string(), e.to_string())),
                "no vector for {s} + {e}"
            );
        }
    }
    assert_eq!(seen.len(), STATES.len() * EVENTS.len());
}

#[test]
fn unknown_is_the_default_rejection() {
    assert_eq!(
        transition(State::Idle, Event::Done, Rules::default()),
        Err(Rejected::Unknown)
    );
}
