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
    /// A to-do the person can link to focus sessions.
    Task,
    Setting,
    /// The live timer state; one row with the key `current`.
    Timer,
}

impl Entity {
    pub fn code(self) -> &'static str {
        match self {
            Entity::Session => "session",
            Entity::Category => "category",
            Entity::Task => "task",
            Entity::Setting => "setting",
            Entity::Timer => "timer",
        }
    }

    pub fn from_code(code: &str) -> Option<Entity> {
        Some(match code {
            "session" => Entity::Session,
            "category" => Entity::Category,
            "task" => Entity::Task,
            "setting" => Entity::Setting,
            "timer" => Entity::Timer,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Add,
    Edit,
    Delete,
}

impl Op {
    pub fn code(self) -> &'static str {
        match self {
            Op::Add => "add",
            Op::Edit => "edit",
            Op::Delete => "delete",
        }
    }

    pub fn from_code(code: &str) -> Option<Op> {
        Some(match code {
            "add" => Op::Add,
            "edit" => Op::Edit,
            "delete" => Op::Delete,
            _ => return None,
        })
    }
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

/// A projected row. Every field remembers the id of the event that last set it, so events can arrive in any
/// order and still end in the same row. A deleted row stays as a tombstone so a late add or edit cannot bring it
/// back; the tombstone holds the highest delete id seen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row {
    pub data: Map<String, Value>,
    pub versions: BTreeMap<String, String>,
    pub tombstone: Option<String>,
}

impl Row {
    pub fn is_deleted(&self) -> bool {
        self.tombstone.is_some()
    }
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
    /// Newer events already set every field this event touches.
    Stale,
    /// The row is deleted; the tombstone wins.
    Superseded,
    Rejected(Reason),
}

pub enum Decision {
    Write(Row, Applied),
    Skip(Applied),
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
            let versions = data.keys().map(|k| (k.clone(), event.id.clone())).collect();
            Decision::Write(
                Row {
                    data,
                    versions,
                    tombstone: None,
                },
                Applied::Added,
            )
        }
        (Op::Edit | Op::Delete, None) => Decision::Skip(Applied::Rejected(Reason::Missing)),
        (Op::Delete, Some(row)) => match &row.tombstone {
            Some(seen) if event.id <= *seen => Decision::Skip(Applied::Superseded),
            _ => Decision::Write(
                Row {
                    data: Map::new(),
                    versions: BTreeMap::new(),
                    tombstone: Some(event.id.clone()),
                },
                Applied::Deleted,
            ),
        },
        (_, Some(row)) if row.is_deleted() => Decision::Skip(Applied::Superseded),
        // Adding an id that exists is an edit. A field changes only if this event is newer than the one that set it.
        (Op::Add | Op::Edit, Some(row)) => {
            let mut next = row.clone();
            for (field, value) in patch.into_iter().flatten() {
                if next
                    .versions
                    .get(field)
                    .is_none_or(|current| event.id > *current)
                {
                    next.data.insert(field.clone(), value.clone());
                    next.versions.insert(field.clone(), event.id.clone());
                }
            }
            if next == *row {
                Decision::Skip(Applied::Stale)
            } else {
                Decision::Write(next, Applied::Edited)
            }
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
        assert!(s.get(Entity::Session, "a").is_some_and(|r| r.is_deleted()));
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
        assert!(row.is_deleted() && row.data.is_empty());
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

    #[test]
    fn older_edit_of_other_fields_still_lands() {
        let mut s = Store::new();
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        s.apply(ev("4", Op::Edit, "a", json!({"b": 4})));
        assert_eq!(
            s.apply(ev("2", Op::Edit, "a", json!({"a": 2}))),
            Applied::Edited
        );
        assert_eq!(field(&s, "a", "a"), Some(json!(2)));
        assert_eq!(field(&s, "a", "b"), Some(json!(4)));
    }

    #[test]
    fn highest_delete_id_is_the_tombstone() {
        let mut s = Store::new();
        s.apply(ev("1", Op::Add, "a", json!({"n": 1})));
        s.apply(ev("5", Op::Delete, "a", Value::Null));
        assert_eq!(
            s.apply(ev("3", Op::Delete, "a", Value::Null)),
            Applied::Superseded
        );
        assert_eq!(
            s.get(Entity::Session, "a")
                .and_then(|r| r.tombstone.clone()),
            Some("5".to_string())
        );
    }

    fn permutations(items: &[EventRecord]) -> Vec<Vec<EventRecord>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let head = rest.remove(i);
            for mut tail in permutations(&rest) {
                tail.insert(0, head.clone());
                out.push(tail);
            }
        }
        out
    }

    fn final_row(events: &[EventRecord]) -> Option<Row> {
        let mut s = Store::new();
        for e in events {
            s.apply(e.clone());
        }
        s.get(Entity::Session, "a").cloned()
    }

    /// Whatever order the events arrive in, the row ends up the same (this is what sync will rely on).
    #[test]
    fn every_arrival_order_converges() {
        let sets: Vec<Vec<EventRecord>> = vec![
            vec![
                ev("1", Op::Add, "a", json!({"x": 1, "y": 1})),
                ev("2", Op::Edit, "a", json!({"x": 2})),
                ev("3", Op::Edit, "a", json!({"y": 3, "z": 3})),
                ev("4", Op::Edit, "a", json!({"x": 4})),
                ev("5", Op::Add, "a", json!({"z": 5})),
            ],
            vec![
                ev("1", Op::Add, "a", json!({"x": 1})),
                ev("2", Op::Edit, "a", json!({"x": 2})),
                ev("3", Op::Delete, "a", Value::Null),
                ev("4", Op::Edit, "a", json!({"x": 4})),
                ev("5", Op::Delete, "a", Value::Null),
            ],
        ];
        for events in sets {
            let expected = final_row(&events);
            assert!(expected.is_some());
            for order in permutations(&events) {
                let ids: Vec<_> = order.iter().map(|e| e.id.as_str()).collect();
                assert_eq!(final_row(&order), expected, "order {ids:?}");
            }
        }
    }
}
