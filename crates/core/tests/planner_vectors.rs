//! Runs the shared planner vectors (test-vectors/planner.json). The TypeScript port runs the same file.
//!
//! Rust is the source of truth for the expected results. After changing the planner on purpose, rerun with
//! `UPDATE_VECTORS=1 cargo test -p op-core --test planner_vectors` and review the diff of the vector file.

use op_core::{Action, Config, PlanError, PlanSettings, Rules, TimerState, plan};
use serde_json::{Map, Value, json};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-vectors/planner.json"
);

fn int(settings: &Map<String, Value>, key: &str) -> i64 {
    settings
        .get(key)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("setting {key}"))
}

fn flag(settings: &Map<String, Value>, key: &str) -> bool {
    settings
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("setting {key}"))
}

fn settings_of(defaults: &Value, overrides: Option<&Value>) -> PlanSettings {
    let mut merged = defaults.as_object().cloned().expect("defaults object");
    if let Some(Value::Object(extra)) = overrides {
        merged.extend(extra.clone());
    }
    let rounds = u32::try_from(int(&merged, "rounds_before_long")).expect("rounds");
    PlanSettings {
        config: Config::new(
            int(&merged, "focus_ms"),
            int(&merged, "short_break_ms"),
            int(&merged, "long_break_ms"),
            rounds,
        )
        .expect("valid config"),
        rules: Rules {
            breathe: flag(&merged, "breathe"),
            auto_start_break: flag(&merged, "auto_start_break"),
            auto_start_focus: flag(&merged, "auto_start_focus"),
            strict: flag(&merged, "strict"),
        },
        breathe_ms: int(&merged, "breathe_ms"),
    }
}

fn action_of(value: &Value) -> Action {
    serde_json::from_value(value.clone()).expect("action")
}

/// What one step produces, in the form stored in the vector file.
fn outcome(
    state: &TimerState,
    step: &Value,
    settings: &PlanSettings,
) -> (Value, Option<TimerState>) {
    let at = step["at"].as_i64().expect("at");
    match plan(state, &action_of(&step["action"]), at, settings) {
        Ok(planned) => {
            let records: Vec<Value> = planned.records.iter().map(|r| r.to_json()).collect();
            (
                json!({ "timer": planned.next.to_json(), "records": records }),
                Some(planned.next),
            )
        }
        Err(PlanError::Invalid(code)) => (json!({ "error": format!("invalid:{code}") }), None),
        Err(PlanError::Rejected(code)) => (json!({ "error": format!("rejected:{code}") }), None),
    }
}

#[test]
fn shared_planner_vectors() {
    let update = std::env::var_os("UPDATE_VECTORS").is_some();
    let mut doc: Value =
        serde_json::from_str(&std::fs::read_to_string(PATH).expect("read")).expect("parse");
    let defaults = doc["defaults"].clone();
    assert!(doc["cases"].as_array().is_some_and(|c| !c.is_empty()));

    for case in doc["cases"].as_array_mut().expect("cases") {
        let name = case["name"].as_str().expect("name").to_string();
        let settings = settings_of(&defaults, case.get("settings"));
        let mut state = TimerState::idle();
        for (i, step) in case["steps"]
            .as_array_mut()
            .expect("steps")
            .iter_mut()
            .enumerate()
        {
            let (got, next) = outcome(&state, step, &settings);
            if update {
                step["expect"] = got;
            } else {
                assert_eq!(step["expect"], got, "{name} step {i}");
            }
            if let Some(next) = next {
                state = next;
            }
        }
    }

    if update {
        let mut text = serde_json::to_string_pretty(&doc).expect("serialise");
        text.push('\n');
        std::fs::write(PATH, text).expect("write vectors");
    }
}
