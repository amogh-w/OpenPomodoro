//! Events and their effect on rows (D-02). The log is append-only; rows are a projection of it.
//!
//! [`decide`] is the single place that says what an event does to a row. The in-memory [`Store`] below uses
//! it, and so does the SQLite layer, so both always agree.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Entity {
    Session,
    Category,
    Setting,
    /// The live timer state; one row with the key `current`.
    Timer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Add,
    Edit,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub entity: Entity,
    pub op: Op,
    /// Which row the event is about (a session id, a setting name, `current` for the timer).
    pub key: String,
    #[serde(default)]
    pub payload: Value,
}

/// A projected row. A deleted row stays as a tombstone so a late add or edit cannot bring it back.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub data: Map<String, Value>,
    /// Id of the last event that changed this row.
    pub version: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Edit or delete for a row that does not exist (yet). Kept so it can be replayed if the add arrives later.
    Missing,
    /// Add or edit whose payload is not a JSON object.
    BadPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Added,
    Edited,
    Deleted,
    /// The event id was already applied; nothing changed.
    Duplicate,
    /// A newer event already changed this row.
    Stale,
    /// The row is deleted; the tombstone wins.
    Superseded,
    Rejected(Reason),
}

pub enum Decision {
    Write(Row, Applied),
    Skip(Applied),
}

fn merged(base: &Map<String, Value>, patch: &Map<String, Value>) -> Map<String, Value> {
    let mut out = base.clone();
    for (k, v) in patch {
        out.insert(k.clone(), v.clone());
    }
    out
}

/// What `event` does to the row it targets. Does not check for duplicate event ids; the log's primary key does.
pub fn decide(existing: Option<&Row>, event: &EventRecord) -> Decision {
    let patch = match (&event.op, &event.payload) {
        (Op::Delete, _) => None,
        (_, Value::Object(map)) => Some(map),
        _ => return Decision::Skip(Applied::Rejected(Reason::BadPayload)),
    };

    match (event.op, existing) {
        (Op::Add, None) => {
            let data = patch.cloned().unwrap_or_default();
            Decision::Write(
                Row {
                    data,
                    version: event.id.clone(),
                    deleted: false,
                },
                Applied::Added,
            )
        }
        (Op::Edit | Op::Delete, None) => Decision::Skip(Applied::Rejected(Reason::Missing)),
        (_, Some(row)) if row.deleted => Decision::Skip(Applied::Superseded),
        (Op::Delete, Some(_)) => Decision::Write(
            Row {
                data: Map::new(),
                version: event.id.clone(),
                deleted: true,
            },
            Applied::Deleted,
        ),
        // Adding an id that exists is an edit. Only a newer event may change the row.
        (Op::Add | Op::Edit, Some(row)) => {
            if event.id <= row.version {
                return Decision::Skip(Applied::Stale);
            }
            let data = merged(&row.data, patch.unwrap_or(&Map::new()));
            Decision::Write(
                Row {
                    data,
                    version: event.id.clone(),
                    deleted: false,
                },
                Applied::Edited,
            )
        }
    }
}

/// In-memory projection of an event log.
#[derive(Debug, Default)]
pub struct Store {
    rows: BTreeMap<(Entity, String), Row>,
    seen: HashSet<String>,
    rejected: Vec<EventRecord>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, entity: Entity, key: &str) -> Option<&Row> {
        self.rows.get(&(entity, key.to_string()))
    }

    /// Edits and deletes that arrived before their add, waiting for it.
    pub fn rejected(&self) -> &[EventRecord] {
        &self.rejected
    }

    pub fn apply(&mut self, event: EventRecord) -> Applied {
        if !self.seen.insert(event.id.clone()) {
            return Applied::Duplicate;
        }
        let slot = (event.entity, event.key.clone());
        match decide(self.rows.get(&slot), &event) {
            Decision::Write(row, applied) => {
                self.rows.insert(slot, row);
                if applied == Applied::Added {
                    self.replay_rejected(event.entity, &event.key);
                }
                applied
            }
            Decision::Skip(applied) => {
                if applied == Applied::Rejected(Reason::Missing) {
                    self.rejected.push(event);
                }
                applied
            }
        }
    }

    /// An add arrived: apply the events that were waiting for it, oldest first.
    fn replay_rejected(&mut self, entity: Entity, key: &str) {
        let (mut waiting, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.rejected)
            .into_iter()
            .partition(|e| e.entity == entity && e.key == key);
        self.rejected = rest;
        waiting.sort_by(|a, b| a.id.cmp(&b.id));
        for event in waiting {
            let slot = (event.entity, event.key.clone());
            if let Decision::Write(row, _) = decide(self.rows.get(&slot), &event) {
                self.rows.insert(slot, row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(id: &str, op: Op, key: &str, payload: Value) -> EventRecord {
        EventRecord {
            id: id.to_string(),
            entity: Entity::Session,
            op,
            key: key.to_string(),
            payload,
        }
    }

    fn field(store: &Store, key: &str, name: &str) -> Option<Value> {
        store
            .get(Entity::Session, key)
            .and_then(|r| r.data.get(name).cloned())
    }

    #[test]
    fn add_then_edit_merges_fields() {
        let mut s = Store::new();
        assert_eq!(
            s.apply(ev(
                "1",
                Op::Add,
                "a",
                json!({"intent": "write", "mood": "neutral"})
            )),
            Applied::Added
        );
        assert_eq!(
            s.apply(ev("2", Op::Edit, "a", json!({"mood": "focused"}))),
            Applied::Edited
        );
        assert_eq!(field(&s, "a", "intent"), Some(json!("write")));
        assert_eq!(field(&s, "a", "mood"), Some(json!("focused")));
    }

    #[test]
    fn same_event_twice_is_a_no_op() {
        let mut s = Store::new();
        let e = ev("1", Op::Add, "a", json!({"n": 1}));
        assert_eq!(s.apply(e.clone()), Applied::Added);
        assert_eq!(s.apply(e), Applied::Duplicate);
    }

    #[test]
    fn add_of_an_existing_id_becomes_an_edit() {
        let mut s = Store::new();
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        assert_eq!(
            s.apply(ev("2", Op::Add, "a", json!({"n": 2}))),
            Applied::Edited
        );
        assert_eq!(field(&s, "a", "n"), Some(json!(2)));
    }

    #[test]
    fn older_edit_arriving_late_is_stale() {
        let mut s = Store::new();
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        s.apply(ev("3", Op::Edit, "a", json!({"n": 3})));
        assert_eq!(
            s.apply(ev("2", Op::Edit, "a", json!({"n": 2}))),
            Applied::Stale
        );
        assert_eq!(field(&s, "a", "n"), Some(json!(3)));
    }

    #[test]
    fn edit_of_a_missing_row_is_rejected_and_kept_then_replayed() {
        let mut s = Store::new();
        assert_eq!(
            s.apply(ev("3", Op::Edit, "a", json!({"n": 3}))),
            Applied::Rejected(Reason::Missing)
        );
        assert_eq!(s.rejected().len(), 1);
        assert_eq!(
            s.apply(ev("1", Op::Add, "a", json!({"n": 1, "k": "v"}))),
            Applied::Added
        );
        assert!(s.rejected().is_empty());
        assert_eq!(
            field(&s, "a", "n"),
            Some(json!(3)),
            "the waiting edit was applied after the add"
        );
        assert_eq!(field(&s, "a", "k"), Some(json!("v")));
    }

    #[test]
    fn delete_before_add_replays_into_a_tombstone() {
        let mut s = Store::new();
        assert_eq!(
            s.apply(ev("3", Op::Delete, "a", Value::Null)),
            Applied::Rejected(Reason::Missing)
        );
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        assert!(s.get(Entity::Session, "a").is_some_and(|r| r.deleted));
    }

    #[test]
    fn tombstone_wins_over_later_add_and_edit() {
        let mut s = Store::new();
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        assert_eq!(
            s.apply(ev("2", Op::Delete, "a", Value::Null)),
            Applied::Deleted
        );
        assert_eq!(
            s.apply(ev("3", Op::Add, "a", json!({"n": 9}))),
            Applied::Superseded
        );
        assert_eq!(
            s.apply(ev("4", Op::Edit, "a", json!({"n": 9}))),
            Applied::Superseded
        );
        let row = s.get(Entity::Session, "a").expect("tombstone stays");
        assert!(row.deleted && row.data.is_empty());
    }

    #[test]
    fn non_object_payload_is_rejected_and_not_kept() {
        let mut s = Store::new();
        assert_eq!(
            s.apply(ev("1", Op::Add, "a", json!("text"))),
            Applied::Rejected(Reason::BadPayload)
        );
        assert!(s.rejected().is_empty());
        assert!(s.get(Entity::Session, "a").is_none());
    }

    #[test]
    fn entities_do_not_share_keys() {
        let mut s = Store::new();
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        let mut other = ev("2", Op::Add, "a", json!({"n": 2}));
        other.entity = Entity::Category;
        assert_eq!(s.apply(other), Applied::Added);
        assert_eq!(field(&s, "a", "n"), Some(json!(1)));
    }
}
