//! SQLite storage (D-01): migrations, WAL, and one transaction per event.
//!
//! `events` is the append-only log and the source of truth. `rows` is its projection, kept up to date in the
//! same transaction by the shared [`op_core::decide`] rules, so a row can always be rebuilt from the log.

use std::fmt;
use std::path::Path;

use op_core::{Applied, Decision, Entity, EventRecord, IdGen, Op, Reason, Row, decide};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value};

/// One entry per schema version; `PRAGMA user_version` records how many have run.
const MIGRATIONS: [&str; 1] = [r#"
    CREATE TABLE events (
        id      TEXT PRIMARY KEY,
        entity  TEXT NOT NULL,
        op      TEXT NOT NULL,
        key     TEXT NOT NULL,
        payload TEXT NOT NULL
    ) WITHOUT ROWID;

    CREATE TABLE rows (
        entity    TEXT NOT NULL,
        key       TEXT NOT NULL,
        data      TEXT NOT NULL,
        versions  TEXT NOT NULL,
        tombstone TEXT,
        PRIMARY KEY (entity, key)
    ) WITHOUT ROWID;

    -- Edits and deletes that arrived before their add; replayed when the add shows up.
    CREATE TABLE rejected_events (
        id     TEXT PRIMARY KEY REFERENCES events(id),
        entity TEXT NOT NULL,
        key    TEXT NOT NULL
    ) WITHOUT ROWID;
    CREATE INDEX rejected_by_row ON rejected_events (entity, key);

    CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) WITHOUT ROWID;

    -- History queries by start time.
    CREATE INDEX session_started ON rows (json_extract(data, '$.started_at')) WHERE entity = 'session';
"#];

#[derive(Debug)]
pub enum DbError {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    /// The file was written by a newer version of the app.
    FutureSchema {
        found: u32,
        supported: u32,
    },
    /// Stored data that does not parse back into a row.
    Corrupt(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sqlite(e) => write!(f, "sqlite: {e}"),
            DbError::Json(e) => write!(f, "json: {e}"),
            DbError::FutureSchema { found, supported } => {
                write!(
                    f,
                    "database schema {found} is newer than this app supports ({supported})"
                )
            }
            DbError::Corrupt(what) => write!(f, "corrupt data: {what}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sqlite(e)
    }
}

impl From<serde_json::Error> for DbError {
    fn from(e: serde_json::Error) -> Self {
        DbError::Json(e)
    }
}

type Result<T> = std::result::Result<T, DbError>;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Db> {
        // `journal_mode` answers with a row, so it has to be read; in-memory databases report "memory".
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = NORMAL;",
        )?;
        migrate(&mut conn)?;
        Ok(Db { conn })
    }

    /// Append `event` to the log and update its row, all in one transaction. An event id that is already in
    /// the log changes nothing and answers `Duplicate`, so retries and outbox replays are safe.
    pub fn apply_event(&mut self, event: &EventRecord) -> Result<Applied> {
        let tx = self.conn.transaction()?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO events (id, entity, op, key, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.id, event.entity.code(), event.op.code(), event.key, serde_json::to_string(&event.payload)?],
        )?;
        if inserted == 0 {
            return Ok(Applied::Duplicate);
        }
        let applied = project(&tx, event)?;
        tx.commit()?;
        Ok(applied)
    }

    /// Throw the projection away and rebuild it from the log. Same result as applying events as they arrived.
    pub fn rebuild_projection(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM rejected_events; DELETE FROM rows;")?;
        let events = {
            let mut stmt =
                tx.prepare("SELECT id, entity, op, key, payload FROM events ORDER BY id")?;
            stmt.query_map([], read_event)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for event in events {
            project(&tx, &event?)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn row(&self, entity: Entity, key: &str) -> Result<Option<Row>> {
        load_row(&self.conn, entity, key)
    }

    /// Live (not deleted) rows of one entity, ordered by key.
    pub fn rows(&self, entity: Entity) -> Result<Vec<(String, Row)>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, data, versions, tombstone FROM rows WHERE entity = ?1 AND tombstone IS NULL ORDER BY key",
        )?;
        let rows = stmt.query_map(params![entity.code()], |r| {
            Ok((r.get::<_, String>(0)?, read_row(r, 1)?))
        })?;
        rows.map(|r| r.map_err(DbError::from).and_then(|(k, row)| Ok((k, row?))))
            .collect()
    }

    /// Live sessions (focus and break) whose `started_at` (Unix ms) is in `from_ms..to_ms`, oldest first.
    pub fn sessions_between(&self, from_ms: i64, to_ms: i64) -> Result<Vec<(String, Row)>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, data, versions, tombstone FROM rows
             WHERE entity = 'session' AND tombstone IS NULL
               AND json_extract(data, '$.started_at') >= ?1 AND json_extract(data, '$.started_at') < ?2
             ORDER BY json_extract(data, '$.started_at'), key",
        )?;
        let rows = stmt.query_map(params![from_ms, to_ms], |r| {
            Ok((r.get::<_, String>(0)?, read_row(r, 1)?))
        })?;
        rows.map(|r| r.map_err(DbError::from).and_then(|(k, row)| Ok((k, row?))))
            .collect()
    }

    /// A setting's value (stored as the `value` field of its row).
    pub fn setting(&self, name: &str) -> Result<Option<Value>> {
        Ok(self
            .row(Entity::Setting, name)?
            .and_then(|row| row.data.get("value").cloned()))
    }

    /// The live timer state, if a phase is in progress.
    pub fn current_timer(&self) -> Result<Option<Map<String, Value>>> {
        Ok(self
            .row(Entity::Timer, "current")?
            .filter(|r| !r.is_deleted())
            .map(|r| r.data))
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    fn meta_get_or_create(&self, key: &str, make: impl FnOnce() -> String) -> Result<String> {
        if let Some(value) = self.meta(key)? {
            return Ok(value);
        }
        let value = make();
        self.conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(self.meta(key)?.unwrap_or(value))
    }

    /// This install's device id, created on first use.
    pub fn device_id(&self) -> Result<String> {
        self.meta_get_or_create("device_id", || uuid::Uuid::new_v4().to_string())
    }

    /// The per-install API token, created on first use.
    pub fn token(&self) -> Result<String> {
        self.meta_get_or_create("token", || {
            format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            )
        })
    }

    /// An id generator that continues after the highest id this device already issued.
    pub fn id_gen(&self) -> Result<IdGen> {
        let device = self.device_id()?;
        let last: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM events WHERE id LIKE '%@' || ?1 ORDER BY id DESC LIMIT 1",
                params![device],
                |r| r.get(0),
            )
            .optional()?;
        Ok(IdGen::restore(&device, last.as_deref()))
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let supported = u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX);
    let found: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if found > supported {
        return Err(DbError::FutureSchema { found, supported });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(found as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        // PRAGMA does not take bound parameters; the number is ours, not user input.
        tx.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
        tx.commit()?;
    }
    Ok(())
}

fn read_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<Result<EventRecord>> {
    let (id, entity, op, key, payload): (String, String, String, String, String) =
        (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
    Ok((|| {
        Ok(EventRecord {
            id,
            entity: Entity::from_code(&entity)
                .ok_or_else(|| DbError::Corrupt(format!("entity {entity}")))?,
            op: Op::from_code(&op).ok_or_else(|| DbError::Corrupt(format!("op {op}")))?,
            key,
            payload: serde_json::from_str(&payload)?,
        })
    })())
}

fn read_row(r: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<Result<Row>> {
    let (data, versions, tombstone): (String, String, Option<String>) =
        (r.get(first)?, r.get(first + 1)?, r.get(first + 2)?);
    Ok((|| {
        Ok(Row {
            data: serde_json::from_str(&data)?,
            versions: serde_json::from_str(&versions)?,
            tombstone,
        })
    })())
}

fn load_row(conn: &Connection, entity: Entity, key: &str) -> Result<Option<Row>> {
    let found = conn
        .query_row(
            "SELECT data, versions, tombstone FROM rows WHERE entity = ?1 AND key = ?2",
            params![entity.code(), key],
            |r| read_row(r, 0),
        )
        .optional()?;
    found.transpose()
}

fn save_row(tx: &Transaction<'_>, entity: Entity, key: &str, row: &Row) -> Result<()> {
    tx.execute(
        "INSERT INTO rows (entity, key, data, versions, tombstone) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (entity, key) DO UPDATE SET data = ?3, versions = ?4, tombstone = ?5",
        params![
            entity.code(),
            key,
            serde_json::to_string(&row.data)?,
            serde_json::to_string(&row.versions)?,
            row.tombstone
        ],
    )?;
    Ok(())
}

/// Apply one logged event to its row, remember it if it has to wait for its add, and replay waiting events
/// after an add.
fn project(tx: &Transaction<'_>, event: &EventRecord) -> Result<Applied> {
    let existing = load_row(tx, event.entity, &event.key)?;
    let applied = match decide(existing.as_ref(), event) {
        Decision::Write(row, applied) => {
            save_row(tx, event.entity, &event.key, &row)?;
            applied
        }
        Decision::Skip(applied) => applied,
    };
    match applied {
        Applied::Rejected(Reason::Missing) => {
            tx.execute(
                "INSERT OR IGNORE INTO rejected_events (id, entity, key) VALUES (?1, ?2, ?3)",
                params![event.id, event.entity.code(), event.key],
            )?;
        }
        Applied::Added => replay_waiting(tx, event.entity, &event.key)?,
        _ => {}
    }
    Ok(applied)
}

fn replay_waiting(tx: &Transaction<'_>, entity: Entity, key: &str) -> Result<()> {
    let waiting = {
        let mut stmt = tx.prepare(
            "SELECT e.id, e.entity, e.op, e.key, e.payload FROM events e
             JOIN rejected_events r ON r.id = e.id
             WHERE r.entity = ?1 AND r.key = ?2 ORDER BY e.id",
        )?;
        stmt.query_map(params![entity.code(), key], read_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    tx.execute(
        "DELETE FROM rejected_events WHERE entity = ?1 AND key = ?2",
        params![entity.code(), key],
    )?;
    for event in waiting {
        let event = event?;
        let existing = load_row(tx, entity, key)?;
        if let Decision::Write(row, _) = decide(existing.as_ref(), &event) {
            save_row(tx, entity, key, &row)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::TempDir;
    use op_core::Store;
    use serde_json::json;

    fn ev(id: &str, entity: Entity, op: Op, key: &str, payload: Value) -> EventRecord {
        EventRecord {
            id: id.to_string(),
            entity,
            op,
            key: key.to_string(),
            payload,
        }
    }

    fn session(id: &str, op: Op, key: &str, payload: Value) -> EventRecord {
        ev(id, Entity::Session, op, key, payload)
    }

    fn log_len(db: &Db) -> i64 {
        db.conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .expect("count")
    }

    #[test]
    fn migrates_a_new_database_and_uses_wal_on_disk() {
        let dir = TempDir::new();
        let path = dir.path().join("op.db");
        let db = Db::open(&path).expect("open");
        let version: u32 = db
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("version");
        assert_eq!(version, 1);
        let mode: String = db
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("mode");
        assert_eq!(mode, "wal");
        drop(db);
        Db::open(&path).expect("reopen is a no-op migration");
    }

    #[test]
    fn refuses_a_database_from_the_future() {
        let dir = TempDir::new();
        let path = dir.path().join("op.db");
        drop(Db::open(&path).expect("open"));
        Connection::open(&path)
            .expect("raw")
            .execute_batch("PRAGMA user_version = 99")
            .expect("bump");
        assert!(matches!(
            Db::open(&path),
            Err(DbError::FutureSchema {
                found: 99,
                supported: 1
            })
        ));
    }

    #[test]
    fn add_edit_delete_round_trip() {
        let mut db = Db::open_in_memory().expect("db");
        assert_eq!(
            db.apply_event(&session(
                "1",
                Op::Add,
                "s1",
                json!({"intent": "write", "started_at": 100})
            ))
            .expect("add"),
            Applied::Added
        );
        assert_eq!(
            db.apply_event(&session("2", Op::Edit, "s1", json!({"mood": "focused"})))
                .expect("edit"),
            Applied::Edited
        );
        let row = db.row(Entity::Session, "s1").expect("row").expect("exists");
        assert_eq!(row.data.get("intent"), Some(&json!("write")));
        assert_eq!(row.data.get("mood"), Some(&json!("focused")));
        assert_eq!(
            db.apply_event(&session("3", Op::Delete, "s1", Value::Null))
                .expect("delete"),
            Applied::Deleted
        );
        assert!(db.rows(Entity::Session).expect("rows").is_empty());
        assert!(
            db.row(Entity::Session, "s1")
                .expect("row")
                .is_some_and(|r| r.is_deleted())
        );
    }

    #[test]
    fn repeating_an_event_changes_nothing() {
        let mut db = Db::open_in_memory().expect("db");
        let e = session("1", Op::Add, "s1", json!({"n": 1}));
        assert_eq!(db.apply_event(&e).expect("first"), Applied::Added);
        assert_eq!(db.apply_event(&e).expect("second"), Applied::Duplicate);
        assert_eq!(log_len(&db), 1);
    }

    #[test]
    fn edit_before_add_waits_then_replays() {
        let mut db = Db::open_in_memory().expect("db");
        let applied = db
            .apply_event(&session("3", Op::Edit, "s1", json!({"mood": "focused"})))
            .expect("edit");
        assert_eq!(applied, Applied::Rejected(Reason::Missing));
        let waiting: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM rejected_events", [], |r| r.get(0))
            .expect("n");
        assert_eq!(waiting, 1);
        db.apply_event(&session("1", Op::Add, "s1", json!({"intent": "write"})))
            .expect("add");
        let row = db.row(Entity::Session, "s1").expect("row").expect("exists");
        assert_eq!(row.data.get("mood"), Some(&json!("focused")));
        let waiting: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM rejected_events", [], |r| r.get(0))
            .expect("n");
        assert_eq!(waiting, 0);
        assert_eq!(log_len(&db), 2, "the log keeps both events");
    }

    #[test]
    fn sessions_between_filters_and_sorts_by_start() {
        let mut db = Db::open_in_memory().expect("db");
        for (i, (key, started)) in [("c", 300), ("a", 100), ("b", 200), ("d", 400)]
            .into_iter()
            .enumerate()
        {
            let id = format!("{}", i + 1);
            db.apply_event(&session(
                &id,
                Op::Add,
                key,
                json!({"started_at": started, "kind": "focus"}),
            ))
            .expect("add");
        }
        db.apply_event(&session("9", Op::Delete, "b", Value::Null))
            .expect("delete");
        let keys: Vec<_> = db
            .sessions_between(100, 400)
            .expect("range")
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(
            keys,
            ["a", "c"],
            "b is deleted, d starts at the excluded end"
        );
    }

    #[test]
    fn setting_and_current_timer_readers() {
        let mut db = Db::open_in_memory().expect("db");
        assert_eq!(db.setting("focus_min").expect("get"), None);
        db.apply_event(&ev(
            "1",
            Entity::Setting,
            Op::Add,
            "focus_min",
            json!({"value": 30}),
        ))
        .expect("set");
        assert_eq!(db.setting("focus_min").expect("get"), Some(json!(30)));

        assert_eq!(db.current_timer().expect("get"), None);
        db.apply_event(&ev(
            "2",
            Entity::Timer,
            Op::Add,
            "current",
            json!({"state": "session", "start": 5}),
        ))
        .expect("t");
        assert_eq!(
            db.current_timer()
                .expect("get")
                .and_then(|m| m.get("state").cloned()),
            Some(json!("session"))
        );
        db.apply_event(&ev("3", Entity::Timer, Op::Delete, "current", Value::Null))
            .expect("clear");
        assert_eq!(db.current_timer().expect("get"), None);
    }

    #[test]
    fn device_id_and_token_are_stable_across_restarts() {
        let dir = TempDir::new();
        let path = dir.path().join("op.db");
        let (device, token) = {
            let db = Db::open(&path).expect("open");
            (db.device_id().expect("device"), db.token().expect("token"))
        };
        let db = Db::open(&path).expect("reopen");
        assert_eq!(db.device_id().expect("device"), device);
        assert_eq!(db.token().expect("token"), token);
        assert!(token.len() >= 64);
    }

    #[test]
    fn id_gen_continues_after_a_restart_even_with_the_clock_behind() {
        let dir = TempDir::new();
        let path = dir.path().join("op.db");
        let first_id = {
            let mut db = Db::open(&path).expect("open");
            let mut id_gen = db.id_gen().expect("gen");
            let id = id_gen.next(9_000_000);
            db.apply_event(&session(&id, Op::Add, "s1", json!({"n": 1})))
                .expect("add");
            id
        };
        let db = Db::open(&path).expect("reopen");
        let next = db.id_gen().expect("gen").next(1_000);
        assert!(next > first_id, "{next} should sort after {first_id}");
    }

    #[test]
    fn rebuilding_the_projection_matches_incremental_application() {
        let mut db = Db::open_in_memory().expect("db");
        let events = [
            session("3", Op::Edit, "a", json!({"y": 3})),
            session("1", Op::Add, "a", json!({"x": 1})),
            session("4", Op::Delete, "b", Value::Null),
            session("2", Op::Add, "b", json!({"x": 2})),
            session("5", Op::Edit, "a", json!({"x": 5})),
        ];
        for e in &events {
            db.apply_event(e).expect("apply");
        }
        let before = (
            db.row(Entity::Session, "a").expect("a"),
            db.row(Entity::Session, "b").expect("b"),
        );
        db.rebuild_projection().expect("rebuild");
        let after = (
            db.row(Entity::Session, "a").expect("a"),
            db.row(Entity::Session, "b").expect("b"),
        );
        assert_eq!(before, after);
        assert!(after.1.is_some_and(|r| r.is_deleted()));
    }

    #[test]
    fn matches_the_in_memory_store_for_any_arrival_order() {
        let events = [
            session("1", Op::Add, "a", json!({"x": 1, "y": 1})),
            session("2", Op::Edit, "a", json!({"x": 2})),
            session("3", Op::Edit, "a", json!({"y": 3, "z": 3})),
            session("4", Op::Delete, "a", Value::Null),
        ];
        let mut store = Store::new();
        for e in &events {
            store.apply(e.clone());
        }
        let expected = store.get(Entity::Session, "a").cloned();

        // A few rotations and the reverse stand in for all orders; core tests every permutation.
        let mut orders: Vec<Vec<usize>> = (0..events.len())
            .map(|r| (0..events.len()).map(|i| (i + r) % events.len()).collect())
            .collect();
        orders.push((0..events.len()).rev().collect());
        for order in orders {
            let mut db = Db::open_in_memory().expect("db");
            for &i in &order {
                db.apply_event(&events[i]).expect("apply");
            }
            assert_eq!(
                db.row(Entity::Session, "a").expect("row"),
                expected,
                "order {order:?}"
            );
        }
    }
}
