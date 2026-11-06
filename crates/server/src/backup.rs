//! Restoring a JSON backup (the file `GET /api/export?format=json` produces) into an account.
//!
//! A backup is read, never trusted: every category, session and setting becomes an ordinary event and goes through
//! the same checks as one typed in the app, anything that does not pass is skipped and counted, and unknown fields
//! are dropped. Items keep their original ids as row keys, so restoring the same file twice changes nothing.

use op_core::{Entity, EventRecord, IdGen, Op};
use serde_json::{Map, Value, json};

use crate::settings;
use crate::validate;

/// Most sessions one restore may bring in; a guard, far above what anyone records by hand.
pub const MAX_SESSIONS: usize = 100_000;
pub const MAX_CATEGORIES: usize = 1_000;

const SESSION_FIELDS: [&str; 10] = [
    "kind",
    "started_at",
    "ended_at",
    "duration_ms",
    "planned_ms",
    "intention",
    "category",
    "task",
    "notes",
    "mood",
];
const CATEGORY_FIELDS: [&str; 3] = ["name", "color", "is_break"];
const TASK_FIELDS: [&str; 4] = ["title", "done", "done_at", "category"];
pub const MAX_TASKS: usize = 20_000;

#[derive(Debug, PartialEq, Eq)]
pub enum BackupError {
    /// Not a file this app wrote (wrong `app`, or a version this build does not know).
    NotABackup,
    TooLarge,
}

#[derive(Debug)]
pub struct Plan {
    pub events: Vec<EventRecord>,
    pub categories: usize,
    pub tasks: usize,
    pub sessions: usize,
    pub settings: usize,
    /// Items that did not pass the checks and were left out.
    pub skipped: usize,
}

/// Keep only the listed fields. A null `intention` means "none", which is stored by leaving it out.
fn pick(source: &Map<String, Value>, fields: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    for field in fields {
        match source.get(*field) {
            None => {}
            Some(Value::Null) if *field == "intention" => {}
            Some(v) => {
                out.insert((*field).to_string(), v.clone());
            }
        }
    }
    out
}

pub fn plan(backup: &Value, ids: &mut IdGen, now_ms: i64) -> Result<Plan, BackupError> {
    if backup.get("app").and_then(Value::as_str) != Some("openpomodoro")
        || backup.get("version").and_then(Value::as_i64) != Some(1)
    {
        return Err(BackupError::NotABackup);
    }
    let list = |name: &str| {
        backup
            .get(name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let (categories, tasks, sessions) = (list("categories"), list("tasks"), list("sessions"));
    if categories.len() > MAX_CATEGORIES || tasks.len() > MAX_TASKS || sessions.len() > MAX_SESSIONS
    {
        return Err(BackupError::TooLarge);
    }

    let mut plan = Plan {
        events: Vec::new(),
        categories: 0,
        tasks: 0,
        sessions: 0,
        settings: 0,
        skipped: 0,
    };
    let mut accept = |plan: &mut Plan, entity: Entity, key: &str, payload: Value| -> bool {
        let event = EventRecord {
            id: ids.next(now_ms),
            entity,
            op: Op::Add,
            key: key.to_string(),
            payload,
        };
        if validate::check_client_event(&event, now_ms).is_ok() {
            plan.events.push(event);
            true
        } else {
            plan.skipped += 1;
            false
        }
    };

    for item in &categories {
        let Some(row) = item.as_object() else {
            plan.skipped += 1;
            continue;
        };
        let Some(key) = row.get("id").and_then(Value::as_str) else {
            plan.skipped += 1;
            continue;
        };
        if accept(
            &mut plan,
            Entity::Category,
            key,
            Value::Object(pick(row, &CATEGORY_FIELDS)),
        ) {
            plan.categories += 1;
        }
    }

    for item in &tasks {
        let Some(row) = item.as_object() else {
            plan.skipped += 1;
            continue;
        };
        let Some(key) = row.get("id").and_then(Value::as_str) else {
            plan.skipped += 1;
            continue;
        };
        if accept(
            &mut plan,
            Entity::Task,
            key,
            Value::Object(pick(row, &TASK_FIELDS)),
        ) {
            plan.tasks += 1;
        }
    }

    if let Some(values) = backup.get("settings").and_then(Value::as_object) {
        for name in settings::NAMES {
            let Some(value) = values.get(name) else {
                continue;
            };
            if accept(&mut plan, Entity::Setting, name, json!({ "value": value })) {
                plan.settings += 1;
            }
        }
    }

    for item in &sessions {
        let Some(row) = item.as_object() else {
            plan.skipped += 1;
            continue;
        };
        let Some(key) = row.get("id").and_then(Value::as_str) else {
            plan.skipped += 1;
            continue;
        };
        if accept(
            &mut plan,
            Entity::Session,
            key,
            Value::Object(pick(row, &SESSION_FIELDS)),
        ) {
            plan.sessions += 1;
        }
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn run(backup: Value) -> Result<Plan, BackupError> {
        plan(&backup, &mut IdGen::new("server"), NOW)
    }

    fn file(extra: Value) -> Value {
        let mut base = json!({"app": "openpomodoro", "version": 1, "categories": [], "sessions": [], "settings": {}});
        if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
            b.extend(e.clone());
        }
        base
    }

    #[test]
    fn only_this_apps_version_one_files_are_accepted() {
        for bad in [
            json!({}),
            json!({"app": "other", "version": 1}),
            json!({"app": "openpomodoro", "version": 2}),
            json!({"app": "openpomodoro"}),
            json!("text"),
            json!(null),
        ] {
            assert_eq!(
                run(bad.clone()).err(),
                Some(BackupError::NotABackup),
                "{bad}"
            );
        }
        assert!(run(file(json!({}))).is_ok());
    }

    #[test]
    fn categories_sessions_and_settings_become_events_keyed_by_their_ids() {
        let p = run(file(json!({
            "categories": [{"id": "c1", "name": "Deep work", "color": "peach", "is_break": false}],
            "settings": {"focus_ms": 1_800_000, "strict": true},
            "sessions": [{"id": "s1", "kind": "focus", "started_at": 1000, "ended_at": 2000, "duration_ms": 1000, "intention": "write", "category": "c1", "notes": null, "mood": "focused"}],
        })))
        .expect("plan");
        assert_eq!(
            (p.categories, p.settings, p.sessions, p.skipped),
            (1, 2, 1, 0)
        );
        let keys: Vec<_> = p
            .events
            .iter()
            .map(|e| (e.entity, e.key.as_str()))
            .collect();
        assert_eq!(
            keys,
            [
                (Entity::Category, "c1"),
                (Entity::Setting, "focus_ms"),
                (Entity::Setting, "strict"),
                (Entity::Session, "s1")
            ]
        );
        assert!(p.events.iter().all(|e| e.op == Op::Add));
    }

    #[test]
    fn event_ids_are_made_here_and_strictly_increasing() {
        let p = run(file(json!({"sessions": [
            {"id": "a", "kind": "focus", "started_at": 1, "ended_at": 2},
            {"id": "b", "kind": "focus", "started_at": 3, "ended_at": 4},
        ]})))
        .expect("plan");
        assert!(p.events[0].id < p.events[1].id);
        assert!(p.events.iter().all(|e| e.id.ends_with("@server")));
    }

    #[test]
    fn a_null_intention_is_left_out_but_other_nulls_are_kept() {
        let p = run(file(json!({"sessions": [{"id": "s", "kind": "focus", "started_at": 1, "ended_at": 2, "intention": null, "category": null, "notes": null, "mood": null}]}))).expect("plan");
        let payload = p.events[0].payload.as_object().expect("object");
        assert!(!payload.contains_key("intention"));
        for field in ["category", "notes", "mood"] {
            assert_eq!(payload.get(field), Some(&Value::Null), "{field}");
        }
    }

    #[test]
    fn unknown_fields_are_dropped_rather_than_carried_in() {
        let p = run(file(json!({"sessions": [{"id": "s", "kind": "focus", "started_at": 1, "ended_at": 2, "surprise": "x", "password_hash": "y"}]}))).expect("plan");
        assert_eq!(p.skipped, 0);
        let payload = p.events[0].payload.as_object().expect("object");
        assert!(!payload.contains_key("surprise") && !payload.contains_key("password_hash"));
    }

    #[test]
    fn bad_items_are_skipped_and_counted_without_stopping_the_rest() {
        let p = run(file(json!({
            "categories": [{"id": "c1", "name": ""}, {"name": "no id"}, "junk", {"id": "ok", "name": "Fine", "color": "blue"}],
            "settings": {"focus_ms": 5, "strict": "yes", "made_up": 1, "breathe": true},
            "sessions": [
                {"id": "s1", "kind": "nap", "started_at": 1, "ended_at": 2},
                {"id": "s2", "kind": "focus", "started_at": 1},
                {"kind": "focus", "started_at": 1, "ended_at": 2},
                {"id": "s4", "kind": "focus", "started_at": 1, "ended_at": 2, "mood": "angry"},
                {"id": "good", "kind": "short_break", "started_at": 5, "ended_at": 6},
                42,
            ],
        })))
        .expect("plan");
        assert_eq!((p.categories, p.settings, p.sessions), (1, 1, 1));
        assert_eq!(p.skipped, 3 + 2 + 5);
    }

    #[test]
    fn settings_outside_the_allowed_range_are_skipped() {
        let p = run(file(
            json!({"settings": {"focus_ms": 999_999_999_999_i64, "rounds_before_long": 0}}),
        ))
        .expect("plan");
        assert_eq!((p.settings, p.skipped), (0, 2));
    }

    #[test]
    fn a_file_with_too_much_in_it_is_refused() {
        let many: Vec<Value> = (0..=MAX_CATEGORIES)
            .map(|i| json!({"id": format!("c{i}"), "name": "x"}))
            .collect();
        assert_eq!(
            run(file(json!({"categories": many}))).err(),
            Some(BackupError::TooLarge)
        );
    }

    #[test]
    fn missing_lists_are_just_empty() {
        let p = run(json!({"app": "openpomodoro", "version": 1})).expect("plan");
        assert!(p.events.is_empty() && p.skipped == 0);
    }
}
