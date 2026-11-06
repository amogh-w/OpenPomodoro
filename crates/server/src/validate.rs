//! Checks for events that come from a client. The log is append-only, so junk must be stopped at the door.

use op_core::{Entity, EventRecord, Op, ids};
use serde_json::{Map, Value};

use crate::settings;

/// How far ahead of the server clock an event id may be. Per-field "newest id wins" means an id from far in the
/// future would win every later edit, so ids more than this ahead are refused. (A guess: one day of clock skew.)
pub const MAX_ID_SKEW_MS: i64 = 24 * 60 * 60 * 1000;
const MAX_KEY_LEN: usize = 100;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
/// Year 2100 in Unix ms: a sanity bound for stored timestamps.
const MAX_TIME_MS: i64 = 4_102_444_800_000;

const ACCENTS: [&str; 12] = [
    "rosewater",
    "flamingo",
    "pink",
    "mauve",
    "red",
    "peach",
    "yellow",
    "green",
    "teal",
    "sky",
    "blue",
    "lavender",
];

/// An id must parse and must not be from the future.
pub fn check_id(id: &str, now_ms: i64) -> Result<(), &'static str> {
    let parsed = ids::parse(id).ok_or("bad_id")?;
    if parsed.secs.saturating_mul(1000) > now_ms.saturating_add(MAX_ID_SKEW_MS) {
        return Err("id_in_future");
    }
    Ok(())
}

fn is_time(v: &Value) -> bool {
    v.as_i64().is_some_and(|n| (0..=MAX_TIME_MS).contains(&n))
}

fn is_text(v: &Value, max_chars: usize) -> bool {
    v.as_str().is_some_and(|s| {
        s.chars().count() <= max_chars && !s.chars().any(|c| c.is_control() && c != '\n')
    })
}

/// Allowed fields of a session row, with a check for each. Everything else is refused.
fn session_field_ok(name: &str, v: &Value) -> Option<bool> {
    Some(match name {
        "kind" => v
            .as_str()
            .is_some_and(|k| matches!(k, "focus" | "short_break" | "long_break")),
        "started_at" | "ended_at" | "duration_ms" | "planned_ms" => is_time(v),
        "intention" => is_text(v, 200),
        "category" => v.is_null() || is_text(v, 64),
        "task" => v.is_null() || is_text(v, 64),
        "notes" => v.is_null() || is_text(v, 4000),
        "mood" => {
            v.is_null()
                || v.as_str()
                    .is_some_and(|m| matches!(m, "focused" | "neutral" | "distracted"))
        }
        _ => return None,
    })
}

/// Allowed fields of a task row.
fn task_field_ok(name: &str, v: &Value) -> Option<bool> {
    Some(match name {
        "title" => v.as_str().is_some_and(|s| {
            (1..=200).contains(&s.chars().count()) && !s.chars().any(char::is_control)
        }),
        "done" => v.is_boolean(),
        "done_at" => v.is_null() || is_time(v),
        "category" => v.is_null() || is_text(v, 64),
        _ => return None,
    })
}

fn category_field_ok(name: &str, v: &Value) -> Option<bool> {
    Some(match name {
        "name" => v
            .as_str()
            .is_some_and(|s| (1..=40).contains(&s.chars().count())),
        "color" => v.as_str().is_some_and(|c| ACCENTS.contains(&c)),
        "is_break" => v.is_boolean(),
        _ => return None,
    })
}

fn check_fields(
    payload: &Map<String, Value>,
    field_ok: fn(&str, &Value) -> Option<bool>,
    required: &[&str],
    is_add: bool,
) -> Result<(), &'static str> {
    for (name, value) in payload {
        match field_ok(name, value) {
            None => return Err("unknown_field"),
            Some(false) => return Err("bad_field"),
            Some(true) => {}
        }
    }
    if is_add && required.iter().any(|r| !payload.contains_key(*r)) {
        return Err("missing_field");
    }
    Ok(())
}

/// Validate an event a client sent. Timer events are never accepted from clients: only the server's own
/// commands write them.
pub fn check_client_event(event: &EventRecord, now_ms: i64) -> Result<(), &'static str> {
    check_id(&event.id, now_ms)?;
    if event.key.is_empty()
        || event.key.chars().count() > MAX_KEY_LEN
        || event.key.chars().any(char::is_control)
    {
        return Err("bad_key");
    }
    if event.entity == Entity::Timer {
        return Err("forbidden_entity");
    }
    if event.op == Op::Delete {
        return Ok(());
    }
    let Value::Object(payload) = &event.payload else {
        return Err("bad_payload");
    };
    if event.payload.to_string().len() > MAX_PAYLOAD_BYTES {
        return Err("payload_too_large");
    }
    let is_add = event.op == Op::Add;
    match event.entity {
        Entity::Setting => {
            if !settings::NAMES.contains(&event.key.as_str()) {
                return Err("unknown_setting");
            }
            let only_value = payload.len() == 1;
            match payload.get("value") {
                Some(v) if only_value && settings::valid_value(&event.key, v) => Ok(()),
                _ => Err("bad_setting_value"),
            }
        }
        Entity::Session => check_fields(
            payload,
            session_field_ok,
            &["kind", "started_at", "ended_at"],
            is_add,
        ),
        Entity::Category => check_fields(payload, category_field_ok, &["name"], is_add),
        Entity::Task => check_fields(payload, task_field_ok, &["title"], is_add),
        Entity::Timer => Err("forbidden_entity"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_core::IdGen;
    use serde_json::json;

    const NOW: i64 = 1_700_000_000_000;

    fn id_at(ms: i64) -> String {
        IdGen::new("dev").next(ms)
    }

    fn ev(entity: Entity, op: Op, key: &str, payload: Value) -> EventRecord {
        EventRecord {
            id: id_at(NOW),
            entity,
            op,
            key: key.into(),
            payload,
        }
    }

    fn session_add(payload: Value) -> EventRecord {
        ev(Entity::Session, Op::Add, "s1", payload)
    }

    #[test]
    fn ids_must_parse_and_not_be_far_in_the_future() {
        assert_eq!(check_id("junk", NOW), Err("bad_id"));
        assert_eq!(check_id(&id_at(NOW), NOW), Ok(()));
        assert_eq!(
            check_id(&id_at(NOW - 30 * 24 * 3_600_000), NOW),
            Ok(()),
            "old offline events are fine"
        );
        assert_eq!(check_id(&id_at(NOW + MAX_ID_SKEW_MS - 2_000), NOW), Ok(()));
        assert_eq!(
            check_id(&id_at(NOW + MAX_ID_SKEW_MS + 5_000), NOW),
            Err("id_in_future")
        );
        assert_eq!(
            check_id(&id_at(250_000_000_000_000), NOW),
            Err("id_in_future")
        );
    }

    #[test]
    fn timer_events_are_never_accepted_from_clients() {
        let e = ev(
            Entity::Timer,
            Op::Add,
            "current",
            json!({"state": "session"}),
        );
        assert_eq!(check_client_event(&e, NOW), Err("forbidden_entity"));
    }

    #[test]
    fn keys_are_bounded() {
        for key in ["", &"k".repeat(101), "a\nb"] {
            let e = ev(Entity::Session, Op::Delete, key, Value::Null);
            assert_eq!(check_client_event(&e, NOW), Err("bad_key"), "{key:?}");
        }
        assert_eq!(
            check_client_event(&ev(Entity::Session, Op::Delete, "s1", Value::Null), NOW),
            Ok(())
        );
    }

    #[test]
    fn settings_must_be_known_and_valid() {
        let ok = ev(
            Entity::Setting,
            Op::Add,
            "focus_ms",
            json!({"value": 1_500_000}),
        );
        assert_eq!(check_client_event(&ok, NOW), Ok(()));
        let unknown = ev(Entity::Setting, Op::Add, "made_up", json!({"value": 1}));
        assert_eq!(check_client_event(&unknown, NOW), Err("unknown_setting"));
        for payload in [
            json!({"value": 5}),
            json!({"value": 1_500_000, "extra": 1}),
            json!({}),
            json!({"value": "x"}),
        ] {
            let e = ev(Entity::Setting, Op::Edit, "focus_ms", payload.clone());
            assert_eq!(
                check_client_event(&e, NOW),
                Err("bad_setting_value"),
                "{payload}"
            );
        }
    }

    #[test]
    fn a_session_add_needs_its_core_fields() {
        let full = json!({"kind": "focus", "started_at": 1000, "ended_at": 2000, "duration_ms": 1000, "intention": "write", "mood": "focused"});
        assert_eq!(check_client_event(&session_add(full), NOW), Ok(()));
        let missing = json!({"kind": "focus", "started_at": 1000});
        assert_eq!(
            check_client_event(&session_add(missing), NOW),
            Err("missing_field")
        );
        let edit = ev(
            Entity::Session,
            Op::Edit,
            "s1",
            json!({"mood": "neutral", "notes": "went well"}),
        );
        assert_eq!(
            check_client_event(&edit, NOW),
            Ok(()),
            "an edit may touch any subset"
        );
    }

    #[test]
    fn session_fields_are_checked() {
        let base = |k: &str, v: Value| {
            session_add(json!({"kind": "focus", "started_at": 1, "ended_at": 2, k: v}))
        };
        assert_eq!(
            check_client_event(&base("kind", json!("nap")), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("mood", json!("angry")), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("started_at", json!(-1)), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("duration_ms", json!("long")), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("intention", json!("x".repeat(201))), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("notes", json!("x".repeat(4001))), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("intention", json!("bell\u{7}")), NOW),
            Err("bad_field")
        );
        assert_eq!(
            check_client_event(&base("surprise", json!(1)), NOW),
            Err("unknown_field")
        );
        assert_eq!(
            check_client_event(&base("notes", json!("line one\nline two")), NOW),
            Ok(())
        );
    }

    #[test]
    fn tasks_are_checked() {
        let ok = ev(
            Entity::Task,
            Op::Add,
            "t1",
            json!({"title": "Write the report", "done": false, "category": null}),
        );
        assert_eq!(check_client_event(&ok, NOW), Ok(()));
        for (payload, want) in [
            (json!({"done": false}), "missing_field"),
            (json!({"title": ""}), "bad_field"),
            (json!({"title": "x".repeat(201)}), "bad_field"),
            (json!({"title": "bell\u{7}"}), "bad_field"),
            (json!({"title": "a", "done": "yes"}), "bad_field"),
            (json!({"title": "a", "colour": 1}), "unknown_field"),
        ] {
            let e = ev(Entity::Task, Op::Add, "t1", payload.clone());
            assert_eq!(check_client_event(&e, NOW), Err(want), "{payload}");
        }
        let done = ev(
            Entity::Task,
            Op::Edit,
            "t1",
            json!({"done": true, "done_at": 1000}),
        );
        assert_eq!(check_client_event(&done, NOW), Ok(()));
    }

    #[test]
    fn categories_are_checked() {
        let ok = ev(
            Entity::Category,
            Op::Add,
            "c1",
            json!({"name": "Writing", "color": "peach", "is_break": false}),
        );
        assert_eq!(check_client_event(&ok, NOW), Ok(()));
        for payload in [
            json!({"color": "peach"}),
            json!({"name": ""}),
            json!({"name": "x", "color": "#ff0000"}),
            json!({"name": "x", "is_break": "no"}),
        ] {
            let e = ev(Entity::Category, Op::Add, "c1", payload.clone());
            assert!(check_client_event(&e, NOW).is_err(), "{payload}");
        }
    }

    #[test]
    fn payloads_must_be_small_objects() {
        assert_eq!(
            check_client_event(&ev(Entity::Session, Op::Add, "s", json!("text")), NOW),
            Err("bad_payload")
        );
        let huge =
            json!({"kind": "focus", "started_at": 1, "ended_at": 2, "notes": "x".repeat(20_000)});
        assert_eq!(
            check_client_event(&session_add(huge), NOW),
            Err("payload_too_large")
        );
    }
}
