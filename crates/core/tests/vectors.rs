//! Runs the shared JSON timer vectors (test-vectors/timer.json). The TypeScript port runs the same file.

use op_core::{Timer, TimerError};
use serde::Deserialize;

#[derive(Deserialize)]
struct Suite {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    ops: Vec<Op>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
    Start {
        at: i64,
        total: i64,
        error: Option<String>,
    },
    Pause {
        at: i64,
        error: Option<String>,
    },
    Resume {
        at: i64,
        error: Option<String>,
    },
    Extend {
        at: i64,
        delta: i64,
    },
    Expect {
        at: i64,
        elapsed: i64,
        remaining: i64,
        overflow: i64,
        ended: bool,
        paused: bool,
    },
}

fn check(case: &str, step: usize, result: Result<(), TimerError>, expected: &Option<String>) {
    match (result, expected) {
        (Ok(()), None) => {}
        (Err(e), Some(code)) => assert_eq!(e.code(), code, "{case} op {step}: wrong error"),
        (Ok(()), Some(code)) => panic!("{case} op {step}: expected error {code}, got Ok"),
        (Err(e), None) => panic!("{case} op {step}: unexpected error {e}"),
    }
}

#[test]
fn shared_timer_vectors() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/timer.json");
    let text = std::fs::read_to_string(path).expect("read vectors");
    let suite: Suite = serde_json::from_str(&text).expect("parse vectors");
    assert!(!suite.cases.is_empty());

    for case in suite.cases {
        let mut timer: Option<Timer> = None;
        for (i, op) in case.ops.iter().enumerate() {
            match op {
                Op::Start { at, total, error } => match Timer::new(*at, *total) {
                    Ok(t) => {
                        check(&case.name, i, Ok(()), error);
                        timer = Some(t);
                    }
                    Err(e) => check(&case.name, i, Err(e), error),
                },
                Op::Pause { at, error } => {
                    let t = timer.as_mut().expect("timer started");
                    check(&case.name, i, t.pause(*at), error);
                }
                Op::Resume { at, error } => {
                    let t = timer.as_mut().expect("timer started");
                    check(&case.name, i, t.resume(*at), error);
                }
                Op::Extend { at, delta } => {
                    timer.as_mut().expect("timer started").extend(*delta, *at)
                }
                Op::Expect {
                    at,
                    elapsed,
                    remaining,
                    overflow,
                    ended,
                    paused,
                } => {
                    let t = timer.as_ref().expect("timer started");
                    let ctx = format!("{} op {i} at {at}", case.name);
                    assert_eq!(t.elapsed(*at), *elapsed, "{ctx}: elapsed");
                    assert_eq!(t.remaining(*at), *remaining, "{ctx}: remaining");
                    assert_eq!(t.overflow(*at), *overflow, "{ctx}: overflow");
                    assert_eq!(t.is_ended(*at), *ended, "{ctx}: ended");
                    assert_eq!(t.is_paused(), *paused, "{ctx}: paused");
                }
            }
        }
    }
}
