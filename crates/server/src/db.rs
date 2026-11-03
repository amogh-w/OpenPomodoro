//! SQLite storage (D-01): migrations, WAL, and one transaction per event.
//!
//! Every row belongs to one user. `events` is that user's append-only log and the source of truth; `rows` is
//! its projection, kept up to date in the same transaction by the shared [`op_core::decide`] rules, so a row
//! can always be rebuilt from the log. Every query below is scoped by user id: one user can neither read nor
//! collide with another's data.

use std::fmt;
use std::path::Path;

use op_core::{Applied, Decision, Entity, EventRecord, IdGen, Op, Reason, Row, decide};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value};

/// One entry per schema version; `PRAGMA user_version` records how many have run.
const MIGRATIONS: [&str; 3] = [
    r#"
    CREATE TABLE users (
        id            TEXT PRIMARY KEY,
        username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
        password_hash TEXT NOT NULL,
        role          TEXT NOT NULL CHECK (role IN ('admin', 'user')),
        disabled      INTEGER NOT NULL DEFAULT 0,
        created_at    INTEGER NOT NULL
    ) WITHOUT ROWID;

    CREATE TABLE events (
        user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        id      TEXT NOT NULL,
        entity  TEXT NOT NULL,
        op      TEXT NOT NULL,
        key     TEXT NOT NULL,
        payload TEXT NOT NULL,
        PRIMARY KEY (user_id, id)
    ) WITHOUT ROWID;

    CREATE TABLE rows (
        user_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        entity    TEXT NOT NULL,
        key       TEXT NOT NULL,
        data      TEXT NOT NULL,
        versions  TEXT NOT NULL,
        tombstone TEXT,
        PRIMARY KEY (user_id, entity, key)
    ) WITHOUT ROWID;

    -- Edits and deletes that arrived before their add; replayed when the add shows up.
    CREATE TABLE rejected_events (
        user_id TEXT NOT NULL,
        id      TEXT NOT NULL,
        entity  TEXT NOT NULL,
        key     TEXT NOT NULL,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id, id) REFERENCES events(user_id, id) ON DELETE CASCADE
    ) WITHOUT ROWID;
    CREATE INDEX rejected_by_row ON rejected_events (user_id, entity, key);

    -- Server-wide values (the server's own device id).
    CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) WITHOUT ROWID;

    -- History queries by start time.
    CREATE INDEX session_started ON rows (user_id, json_extract(data, '$.started_at')) WHERE entity = 'session';
"#,
    r#"
    -- Sign-in sessions. Only a hash of the cookie value is stored, so a leaked database cannot be replayed.
    CREATE TABLE auth_sessions (
        token_hash TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        last_seen  INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    ) WITHOUT ROWID;
    CREATE INDEX auth_sessions_by_user ON auth_sessions (user_id);

    -- Failed sign-ins per account and per address, for backoff.
    CREATE TABLE login_attempts (
        key          TEXT PRIMARY KEY,
        failures     INTEGER NOT NULL,
        locked_until INTEGER NOT NULL,
        updated_at   INTEGER NOT NULL
    ) WITHOUT ROWID;
"#,
    r#"
    -- Command ids already processed, so a retried or replayed command does nothing the second time.
    CREATE TABLE commands_seen (
        user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        id      TEXT NOT NULL,
        PRIMARY KEY (user_id, id)
    ) WITHOUT ROWID;
"#,
];

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
    /// Another account already uses this username (compared without regard to case).
    UsernameTaken,
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
            DbError::UsernameTaken => write!(f, "username is already taken"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    User,
}

impl Role {
    pub fn code(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::User => "user",
        }
    }

    fn from_code(code: &str) -> Option<Role> {
        match code {
            "admin" => Some(Role::Admin),
            "user" => Some(Role::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub disabled: bool,
    /// Unix milliseconds.
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    pub user_id: String,
    pub last_seen: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt {
    pub failures: u32,
    pub locked_until: i64,
    pub updated_at: i64,
}

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

    // ---- users ----

    pub fn insert_user(&self, user: &User) -> Result<()> {
        let result = self.conn.execute(
            "INSERT INTO users (id, username, password_hash, role, disabled, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![user.id, user.username, user.password_hash, user.role.code(), user.disabled, user.created_at],
        );
        match result {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, Some(msg)))
                if err.code == rusqlite::ErrorCode::ConstraintViolation
                    && msg.contains("users.username") =>
            {
                Err(DbError::UsernameTaken)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn user_by_id(&self, id: &str) -> Result<Option<User>> {
        self.query_user("WHERE id = ?1", id)
    }

    /// Usernames are compared without regard to case.
    pub fn user_by_name(&self, username: &str) -> Result<Option<User>> {
        self.query_user("WHERE username = ?1", username)
    }

    fn query_user(&self, filter: &str, arg: &str) -> Result<Option<User>> {
        let sql = format!(
            "SELECT id, username, password_hash, role, disabled, created_at FROM users {filter}"
        );
        let found = self
            .conn
            .query_row(&sql, params![arg], read_user)
            .optional()?;
        found.transpose()
    }

    /// All accounts, oldest first.
    pub fn list_users(&self) -> Result<Vec<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, password_hash, role, disabled, created_at FROM users ORDER BY created_at, id",
        )?;
        let users = stmt.query_map([], read_user)?;
        users
            .map(|u| u.map_err(DbError::from).and_then(|u| u))
            .collect()
    }

    pub fn count_users(&self) -> Result<u64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    pub fn set_password_hash(&self, id: &str, hash: &str) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE users SET password_hash = ?2 WHERE id = ?1",
            params![id, hash],
        )? > 0)
    }

    pub fn set_disabled(&self, id: &str, disabled: bool) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE users SET disabled = ?2 WHERE id = ?1",
            params![id, disabled],
        )? > 0)
    }

    /// Delete an account and, by cascade, its whole log and projection.
    pub fn delete_user(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM users WHERE id = ?1", params![id])?
            > 0)
    }

    /// Insert `user` only if there are no accounts yet, atomically, so two first-run setups cannot both win.
    pub fn insert_user_if_none(&self, user: &User) -> Result<bool> {
        let n = self.conn.execute(
            "INSERT INTO users (id, username, password_hash, role, disabled, created_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6 WHERE NOT EXISTS (SELECT 1 FROM users)",
            params![
                user.id,
                user.username,
                user.password_hash,
                user.role.code(),
                user.disabled,
                user.created_at
            ],
        )?;
        Ok(n > 0)
    }

    pub fn count_enabled_admins(&self) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND disabled = 0",
            [],
            |r| r.get(0),
        )?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    // ---- sign-in sessions and attempts ----

    pub fn insert_auth_session(
        &self,
        token_hash: &str,
        user_id: &str,
        now_ms: i64,
        expires_at: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO auth_sessions (token_hash, user_id, created_at, last_seen, expires_at) VALUES (?1, ?2, ?3, ?3, ?4)",
            params![token_hash, user_id, now_ms, expires_at],
        )?;
        Ok(())
    }

    pub fn auth_session(&self, token_hash: &str) -> Result<Option<AuthSession>> {
        Ok(self
            .conn
            .query_row(
                "SELECT user_id, last_seen, expires_at FROM auth_sessions WHERE token_hash = ?1",
                params![token_hash],
                |r| {
                    Ok(AuthSession {
                        user_id: r.get(0)?,
                        last_seen: r.get(1)?,
                        expires_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn touch_auth_session(&self, token_hash: &str, now_ms: i64, expires_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE auth_sessions SET last_seen = ?2, expires_at = ?3 WHERE token_hash = ?1",
            params![token_hash, now_ms, expires_at],
        )?;
        Ok(())
    }

    pub fn delete_auth_session(&self, token_hash: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM auth_sessions WHERE token_hash = ?1",
            params![token_hash],
        )?;
        Ok(())
    }

    /// Sign a user out everywhere, optionally keeping one session.
    pub fn delete_auth_sessions_of(&self, user_id: &str, keep_hash: Option<&str>) -> Result<()> {
        self.conn.execute(
            "DELETE FROM auth_sessions WHERE user_id = ?1 AND token_hash IS NOT ?2",
            params![user_id, keep_hash],
        )?;
        Ok(())
    }

    pub fn purge_expired_auth_sessions(&self, now_ms: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM auth_sessions WHERE expires_at <= ?1",
            params![now_ms],
        )?;
        Ok(())
    }

    pub fn attempt(&self, key: &str) -> Result<Option<Attempt>> {
        Ok(self
            .conn
            .query_row(
                "SELECT failures, locked_until, updated_at FROM login_attempts WHERE key = ?1",
                params![key],
                |r| {
                    Ok(Attempt {
                        failures: r.get(0)?,
                        locked_until: r.get(1)?,
                        updated_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn put_attempt(&self, key: &str, attempt: Attempt) -> Result<()> {
        self.conn.execute(
            "INSERT INTO login_attempts (key, failures, locked_until, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (key) DO UPDATE SET failures = ?2, locked_until = ?3, updated_at = ?4",
            params![key, attempt.failures, attempt.locked_until, attempt.updated_at],
        )?;
        Ok(())
    }

    pub fn delete_attempt(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM login_attempts WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Forget attempt records not touched since `before_ms` (keeps the table from growing without bound).
    pub fn purge_attempts_before(&self, before_ms: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM login_attempts WHERE updated_at < ?1 AND locked_until < ?1",
            params![before_ms],
        )?;
        Ok(())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- events and rows (always for one user) ----

    /// Append `event` to the user's log and update its row, all in one transaction. An event id that is
    /// already in this user's log changes nothing and answers `Duplicate`, so retries and outbox replays are
    /// safe. Other users' logs are never consulted.
    pub fn apply_event(&mut self, user: &str, event: &EventRecord) -> Result<Applied> {
        let tx = self.conn.transaction()?;
        let applied = insert_and_project(&tx, user, event)?;
        if applied != Applied::Duplicate {
            tx.commit()?;
        }
        Ok(applied)
    }

    /// Has this command id already been processed for this user?
    pub fn command_seen(&self, user: &str, command_id: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM commands_seen WHERE user_id = ?1 AND id = ?2",
            params![user, command_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Apply the events a command produced as one unit: all of them or none, and only if this command id has
    /// not been processed before. Answers `None` for a command seen already (nothing is written).
    pub fn apply_command(
        &mut self,
        user: &str,
        command_id: &str,
        events: &[EventRecord],
    ) -> Result<Option<Vec<Applied>>> {
        let tx = self.conn.transaction()?;
        let fresh = tx.execute(
            "INSERT OR IGNORE INTO commands_seen (user_id, id) VALUES (?1, ?2)",
            params![user, command_id],
        )?;
        if fresh == 0 {
            return Ok(None);
        }
        let results = events
            .iter()
            .map(|e| insert_and_project(&tx, user, e))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(Some(results))
    }

    /// Throw every projection away and rebuild it from the logs. Same result as applying events as they arrived.
    pub fn rebuild_projection(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM rejected_events; DELETE FROM rows;")?;
        let events = {
            let mut stmt = tx.prepare(
                "SELECT user_id, id, entity, op, key, payload FROM events ORDER BY user_id, id",
            )?;
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, read_event(r, 1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for (user, event) in events {
            project(&tx, &user, &event?)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn row(&self, user: &str, entity: Entity, key: &str) -> Result<Option<Row>> {
        load_row(&self.conn, user, entity, key)
    }

    /// The user's live (not deleted) rows of one entity, ordered by key.
    pub fn rows(&self, user: &str, entity: Entity) -> Result<Vec<(String, Row)>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, data, versions, tombstone FROM rows
             WHERE user_id = ?1 AND entity = ?2 AND tombstone IS NULL ORDER BY key",
        )?;
        let rows = stmt.query_map(params![user, entity.code()], |r| {
            Ok((r.get::<_, String>(0)?, read_row(r, 1)?))
        })?;
        rows.map(|r| r.map_err(DbError::from).and_then(|(k, row)| Ok((k, row?))))
            .collect()
    }

    /// The user's live sessions (focus and break) whose `started_at` (Unix ms) is in `from_ms..to_ms`, oldest first.
    pub fn sessions_between(
        &self,
        user: &str,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<Vec<(String, Row)>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, data, versions, tombstone FROM rows
             WHERE user_id = ?1 AND entity = 'session' AND tombstone IS NULL
               AND json_extract(data, '$.started_at') >= ?2 AND json_extract(data, '$.started_at') < ?3
             ORDER BY json_extract(data, '$.started_at'), key",
        )?;
        let rows = stmt.query_map(params![user, from_ms, to_ms], |r| {
            Ok((r.get::<_, String>(0)?, read_row(r, 1)?))
        })?;
        rows.map(|r| r.map_err(DbError::from).and_then(|(k, row)| Ok((k, row?))))
            .collect()
    }

    /// A setting's value (stored as the `value` field of its row).
    pub fn setting(&self, user: &str, name: &str) -> Result<Option<Value>> {
        Ok(self
            .row(user, Entity::Setting, name)?
            .and_then(|row| row.data.get("value").cloned()))
    }

    /// The user's live timer state, if a phase is in progress.
    pub fn current_timer(&self, user: &str) -> Result<Option<Map<String, Value>>> {
        Ok(self
            .row(user, Entity::Timer, "current")?
            .filter(|r| !r.is_deleted())
            .map(|r| r.data))
    }

    // ---- server-wide ----

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// This server's device id, used for events the server itself writes. Created on first use.
    pub fn device_id(&self) -> Result<String> {
        if let Some(value) = self.meta("device_id")? {
            return Ok(value);
        }
        let value = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('device_id', ?1)",
            params![value],
        )?;
        Ok(self.meta("device_id")?.unwrap_or(value))
    }

    /// An id generator for server-written events, continuing after the highest id this server already issued
    /// (across all users, so the sequence never goes backwards).
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

fn read_user(r: &rusqlite::Row<'_>) -> rusqlite::Result<Result<User>> {
    let role: String = r.get(3)?;
    let (id, username, password_hash, disabled, created_at) =
        (r.get(0)?, r.get(1)?, r.get(2)?, r.get(4)?, r.get(5)?);
    Ok(Role::from_code(&role)
        .map(|role| User {
            id,
            username,
            password_hash,
            role,
            disabled,
            created_at,
        })
        .ok_or_else(|| DbError::Corrupt(format!("role {role}"))))
}

/// Read an event from columns `first..first + 5` (id, entity, op, key, payload).
fn read_event(r: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<Result<EventRecord>> {
    let (id, entity, op, key, payload): (String, String, String, String, String) = (
        r.get(first)?,
        r.get(first + 1)?,
        r.get(first + 2)?,
        r.get(first + 3)?,
        r.get(first + 4)?,
    );
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

fn load_row(conn: &Connection, user: &str, entity: Entity, key: &str) -> Result<Option<Row>> {
    let found = conn
        .query_row(
            "SELECT data, versions, tombstone FROM rows WHERE user_id = ?1 AND entity = ?2 AND key = ?3",
            params![user, entity.code(), key],
            |r| read_row(r, 0),
        )
        .optional()?;
    found.transpose()
}

fn save_row(tx: &Transaction<'_>, user: &str, entity: Entity, key: &str, row: &Row) -> Result<()> {
    tx.execute(
        "INSERT INTO rows (user_id, entity, key, data, versions, tombstone) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (user_id, entity, key) DO UPDATE SET data = ?4, versions = ?5, tombstone = ?6",
        params![
            user,
            entity.code(),
            key,
            serde_json::to_string(&row.data)?,
            serde_json::to_string(&row.versions)?,
            row.tombstone
        ],
    )?;
    Ok(())
}

/// Log `event` and project it. A repeated event id writes nothing and answers `Duplicate`.
fn insert_and_project(tx: &Transaction<'_>, user: &str, event: &EventRecord) -> Result<Applied> {
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO events (user_id, id, entity, op, key, payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            user,
            event.id,
            event.entity.code(),
            event.op.code(),
            event.key,
            serde_json::to_string(&event.payload)?
        ],
    )?;
    if inserted == 0 {
        return Ok(Applied::Duplicate);
    }
    project(tx, user, event)
}

/// Apply one logged event to its row, remember it if it has to wait for its add, and replay waiting events
/// after an add.
fn project(tx: &Transaction<'_>, user: &str, event: &EventRecord) -> Result<Applied> {
    let existing = load_row(tx, user, event.entity, &event.key)?;
    let applied = match decide(existing.as_ref(), event) {
        Decision::Write(row, applied) => {
            save_row(tx, user, event.entity, &event.key, &row)?;
            applied
        }
        Decision::Skip(applied) => applied,
    };
    match applied {
        Applied::Rejected(Reason::Missing) => {
            tx.execute(
                "INSERT OR IGNORE INTO rejected_events (user_id, id, entity, key) VALUES (?1, ?2, ?3, ?4)",
                params![user, event.id, event.entity.code(), event.key],
            )?;
        }
        Applied::Added => replay_waiting(tx, user, event.entity, &event.key)?,
        _ => {}
    }
    Ok(applied)
}

fn replay_waiting(tx: &Transaction<'_>, user: &str, entity: Entity, key: &str) -> Result<()> {
    let waiting = {
        let mut stmt = tx.prepare(
            "SELECT e.id, e.entity, e.op, e.key, e.payload FROM events e
             JOIN rejected_events r ON r.user_id = e.user_id AND r.id = e.id
             WHERE r.user_id = ?1 AND r.entity = ?2 AND r.key = ?3 ORDER BY e.id",
        )?;
        stmt.query_map(params![user, entity.code(), key], |r| read_event(r, 0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    tx.execute(
        "DELETE FROM rejected_events WHERE user_id = ?1 AND entity = ?2 AND key = ?3",
        params![user, entity.code(), key],
    )?;
    for event in waiting {
        let event = event?;
        let existing = load_row(tx, user, entity, key)?;
        if let Decision::Write(row, _) = decide(existing.as_ref(), &event) {
            save_row(tx, user, entity, key, &row)?;
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

    const ALICE: &str = "user-alice";
    const BOB: &str = "user-bob";

    fn user(id: &str, name: &str) -> User {
        User {
            id: id.to_string(),
            username: name.to_string(),
            password_hash: "hash".to_string(),
            role: Role::User,
            disabled: false,
            created_at: 1,
        }
    }

    /// An in-memory database with accounts for Alice and Bob.
    fn db() -> Db {
        let db = Db::open_in_memory().expect("db");
        db.insert_user(&user(ALICE, "alice")).expect("alice");
        db.insert_user(&user(BOB, "bob")).expect("bob");
        db
    }

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

    fn count(db: &Db, table: &str, user_id: &str) -> i64 {
        db.conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE user_id = ?1"),
                params![user_id],
                |r| r.get(0),
            )
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
        assert_eq!(version, 3);
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
                supported: 3
            })
        ));
    }

    // ---- users ----

    #[test]
    fn users_round_trip_and_usernames_are_unique_ignoring_case() {
        let db = Db::open_in_memory().expect("db");
        db.insert_user(&user("u1", "Alice")).expect("insert");
        assert_eq!(
            db.user_by_name("alice").expect("by name").map(|u| u.id),
            Some("u1".to_string())
        );
        assert_eq!(
            db.user_by_id("u1").expect("by id").map(|u| u.username),
            Some("Alice".to_string())
        );
        assert!(matches!(
            db.insert_user(&user("u2", "ALICE")),
            Err(DbError::UsernameTaken)
        ));
        assert_eq!(db.count_users().expect("count"), 1);
        assert_eq!(db.user_by_name("nobody").expect("none"), None);
    }

    #[test]
    fn user_updates_and_listing() {
        let db = db();
        assert!(db.set_password_hash(ALICE, "new-hash").expect("set"));
        assert!(db.set_disabled(BOB, true).expect("disable"));
        assert!(!db.set_disabled("missing", true).expect("missing"));
        let users = db.list_users().expect("list");
        assert_eq!(users.len(), 2);
        assert_eq!(
            db.user_by_id(ALICE).expect("a").map(|u| u.password_hash),
            Some("new-hash".to_string())
        );
        assert!(db.user_by_id(BOB).expect("b").is_some_and(|u| u.disabled));
    }

    #[test]
    fn events_for_an_unknown_user_are_refused() {
        let mut db = db();
        assert!(
            db.apply_event("ghost", &session("1", Op::Add, "s1", json!({"n": 1})))
                .is_err()
        );
    }

    // ---- events and rows ----

    #[test]
    fn add_edit_delete_round_trip() {
        let mut db = db();
        assert_eq!(
            db.apply_event(
                ALICE,
                &session(
                    "1",
                    Op::Add,
                    "s1",
                    json!({"intent": "write", "started_at": 100})
                )
            )
            .expect("add"),
            Applied::Added
        );
        assert_eq!(
            db.apply_event(
                ALICE,
                &session("2", Op::Edit, "s1", json!({"mood": "focused"}))
            )
            .expect("edit"),
            Applied::Edited
        );
        let row = db
            .row(ALICE, Entity::Session, "s1")
            .expect("row")
            .expect("exists");
        assert_eq!(row.data.get("intent"), Some(&json!("write")));
        assert_eq!(row.data.get("mood"), Some(&json!("focused")));
        assert_eq!(
            db.apply_event(ALICE, &session("3", Op::Delete, "s1", Value::Null))
                .expect("delete"),
            Applied::Deleted
        );
        assert!(db.rows(ALICE, Entity::Session).expect("rows").is_empty());
        assert!(
            db.row(ALICE, Entity::Session, "s1")
                .expect("row")
                .is_some_and(|r| r.is_deleted())
        );
    }

    #[test]
    fn repeating_an_event_changes_nothing() {
        let mut db = db();
        let e = session("1", Op::Add, "s1", json!({"n": 1}));
        assert_eq!(db.apply_event(ALICE, &e).expect("first"), Applied::Added);
        assert_eq!(
            db.apply_event(ALICE, &e).expect("second"),
            Applied::Duplicate
        );
        assert_eq!(count(&db, "events", ALICE), 1);
    }

    #[test]
    fn a_command_is_applied_once_and_as_a_unit() {
        let mut db = db();
        let events = [
            ev(
                "1",
                Entity::Timer,
                Op::Add,
                "current",
                json!({"state": "session"}),
            ),
            session("2", Op::Add, "s1", json!({"started_at": 5})),
        ];
        let first = db.apply_command(ALICE, "cmd-1", &events).expect("apply");
        assert_eq!(first, Some(vec![Applied::Added, Applied::Added]));

        // The same command again writes nothing, even if its events differ.
        let other = [session("3", Op::Add, "s2", json!({"started_at": 6}))];
        assert_eq!(
            db.apply_command(ALICE, "cmd-1", &other).expect("again"),
            None
        );
        assert_eq!(db.row(ALICE, Entity::Session, "s2").expect("row"), None);
        assert_eq!(count(&db, "events", ALICE), 2);

        // Another user's identical command id is independent.
        assert!(
            db.apply_command(BOB, "cmd-1", &other)
                .expect("bob")
                .is_some()
        );
    }

    #[test]
    fn edit_before_add_waits_then_replays() {
        let mut db = db();
        let applied = db
            .apply_event(
                ALICE,
                &session("3", Op::Edit, "s1", json!({"mood": "focused"})),
            )
            .expect("edit");
        assert_eq!(applied, Applied::Rejected(Reason::Missing));
        assert_eq!(count(&db, "rejected_events", ALICE), 1);
        db.apply_event(
            ALICE,
            &session("1", Op::Add, "s1", json!({"intent": "write"})),
        )
        .expect("add");
        let row = db
            .row(ALICE, Entity::Session, "s1")
            .expect("row")
            .expect("exists");
        assert_eq!(row.data.get("mood"), Some(&json!("focused")));
        assert_eq!(count(&db, "rejected_events", ALICE), 0);
        assert_eq!(count(&db, "events", ALICE), 2, "the log keeps both events");
    }

    #[test]
    fn sessions_between_filters_and_sorts_by_start() {
        let mut db = db();
        for (i, (key, started)) in [("c", 300), ("a", 100), ("b", 200), ("d", 400)]
            .into_iter()
            .enumerate()
        {
            let id = format!("{}", i + 1);
            db.apply_event(
                ALICE,
                &session(
                    &id,
                    Op::Add,
                    key,
                    json!({"started_at": started, "kind": "focus"}),
                ),
            )
            .expect("add");
        }
        db.apply_event(ALICE, &session("9", Op::Delete, "b", Value::Null))
            .expect("delete");
        let keys: Vec<_> = db
            .sessions_between(ALICE, 100, 400)
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
        let mut db = db();
        assert_eq!(db.setting(ALICE, "focus_min").expect("get"), None);
        db.apply_event(
            ALICE,
            &ev(
                "1",
                Entity::Setting,
                Op::Add,
                "focus_min",
                json!({"value": 30}),
            ),
        )
        .expect("set");
        assert_eq!(
            db.setting(ALICE, "focus_min").expect("get"),
            Some(json!(30))
        );

        assert_eq!(db.current_timer(ALICE).expect("get"), None);
        db.apply_event(
            ALICE,
            &ev(
                "2",
                Entity::Timer,
                Op::Add,
                "current",
                json!({"state": "session", "start": 5}),
            ),
        )
        .expect("t");
        assert_eq!(
            db.current_timer(ALICE)
                .expect("get")
                .and_then(|m| m.get("state").cloned()),
            Some(json!("session"))
        );
        db.apply_event(
            ALICE,
            &ev("3", Entity::Timer, Op::Delete, "current", Value::Null),
        )
        .expect("clear");
        assert_eq!(db.current_timer(ALICE).expect("get"), None);
    }

    // ---- isolation ----

    #[test]
    fn users_never_see_each_others_rows() {
        let mut db = db();
        db.apply_event(
            ALICE,
            &session(
                "1",
                Op::Add,
                "s1",
                json!({"started_at": 100, "intent": "secret"}),
            ),
        )
        .expect("a");
        db.apply_event(
            ALICE,
            &ev(
                "2",
                Entity::Setting,
                Op::Add,
                "theme",
                json!({"value": "dark"}),
            ),
        )
        .expect("a");
        db.apply_event(
            ALICE,
            &ev(
                "3",
                Entity::Timer,
                Op::Add,
                "current",
                json!({"state": "session"}),
            ),
        )
        .expect("a");

        assert_eq!(db.row(BOB, Entity::Session, "s1").expect("row"), None);
        assert!(db.rows(BOB, Entity::Session).expect("rows").is_empty());
        assert!(
            db.sessions_between(BOB, 0, 1_000)
                .expect("range")
                .is_empty()
        );
        assert_eq!(db.setting(BOB, "theme").expect("setting"), None);
        assert_eq!(db.current_timer(BOB).expect("timer"), None);
        assert_eq!(db.rows(ALICE, Entity::Session).expect("rows").len(), 1);
    }

    #[test]
    fn the_same_event_id_and_key_can_exist_for_two_users() {
        let mut db = db();
        let e = session("1", Op::Add, "s1", json!({"intent": "alice"}));
        assert_eq!(db.apply_event(ALICE, &e).expect("a"), Applied::Added);
        // Bob sending Alice's event id is not a duplicate and cannot probe whether it exists.
        let theirs = session("1", Op::Add, "s1", json!({"intent": "bob"}));
        assert_eq!(db.apply_event(BOB, &theirs).expect("b"), Applied::Added);
        let a = db
            .row(ALICE, Entity::Session, "s1")
            .expect("row")
            .expect("a");
        let b = db.row(BOB, Entity::Session, "s1").expect("row").expect("b");
        assert_eq!(a.data.get("intent"), Some(&json!("alice")));
        assert_eq!(b.data.get("intent"), Some(&json!("bob")));
    }

    #[test]
    fn a_waiting_event_is_replayed_only_for_its_own_user() {
        let mut db = db();
        db.apply_event(
            ALICE,
            &session("3", Op::Edit, "s1", json!({"mood": "focused"})),
        )
        .expect("waits");
        db.apply_event(BOB, &session("1", Op::Add, "s1", json!({"intent": "bob"})))
            .expect("bob adds");
        let bob = db
            .row(BOB, Entity::Session, "s1")
            .expect("row")
            .expect("exists");
        assert_eq!(
            bob.data.get("mood"),
            None,
            "Alice's waiting edit must not land on Bob's row"
        );
        assert_eq!(count(&db, "rejected_events", ALICE), 1);
    }

    #[test]
    fn deleting_a_user_removes_everything_of_theirs_and_nothing_else() {
        let mut db = db();
        db.apply_event(ALICE, &session("1", Op::Add, "s1", json!({"n": 1})))
            .expect("a");
        db.apply_event(ALICE, &session("9", Op::Edit, "missing", json!({"n": 1})))
            .expect("waits");
        db.apply_event(BOB, &session("1", Op::Add, "s1", json!({"n": 2})))
            .expect("b");
        assert!(db.delete_user(ALICE).expect("delete"));
        for table in ["events", "rows", "rejected_events"] {
            assert_eq!(count(&db, table, ALICE), 0, "{table}");
        }
        assert_eq!(db.rows(BOB, Entity::Session).expect("rows").len(), 1);
        assert_eq!(db.user_by_id(ALICE).expect("gone"), None);
    }

    // ---- server device id ----

    #[test]
    fn device_id_is_stable_across_restarts() {
        let dir = TempDir::new();
        let path = dir.path().join("op.db");
        let device = Db::open(&path).expect("open").device_id().expect("device");
        assert_eq!(
            Db::open(&path)
                .expect("reopen")
                .device_id()
                .expect("device"),
            device
        );
    }

    #[test]
    fn id_gen_continues_after_a_restart_even_with_the_clock_behind() {
        let dir = TempDir::new();
        let path = dir.path().join("op.db");
        let first_id = {
            let mut db = Db::open(&path).expect("open");
            db.insert_user(&user(ALICE, "alice")).expect("alice");
            let mut id_gen = db.id_gen().expect("gen");
            let id = id_gen.next(9_000_000);
            db.apply_event(ALICE, &session(&id, Op::Add, "s1", json!({"n": 1})))
                .expect("add");
            id
        };
        let db = Db::open(&path).expect("reopen");
        let next = db.id_gen().expect("gen").next(1_000);
        assert!(next > first_id, "{next} should sort after {first_id}");
    }

    // ---- convergence ----

    #[test]
    fn rebuilding_the_projection_matches_incremental_application() {
        let mut db = db();
        let events = [
            session("3", Op::Edit, "a", json!({"y": 3})),
            session("1", Op::Add, "a", json!({"x": 1})),
            session("4", Op::Delete, "b", Value::Null),
            session("2", Op::Add, "b", json!({"x": 2})),
            session("5", Op::Edit, "a", json!({"x": 5})),
        ];
        for e in &events {
            db.apply_event(ALICE, e).expect("apply");
            db.apply_event(BOB, e).expect("apply");
        }
        let snapshot = |db: &Db| {
            [ALICE, BOB].map(|u| {
                (
                    db.row(u, Entity::Session, "a").expect("a"),
                    db.row(u, Entity::Session, "b").expect("b"),
                )
            })
        };
        let before = snapshot(&db);
        db.rebuild_projection().expect("rebuild");
        assert_eq!(before, snapshot(&db));
        assert!(before[0].1.as_ref().is_some_and(|r| r.is_deleted()));
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
            let mut db = db();
            for &i in &order {
                db.apply_event(ALICE, &events[i]).expect("apply");
            }
            assert_eq!(
                db.row(ALICE, Entity::Session, "a").expect("row"),
                expected,
                "order {order:?}"
            );
        }
    }
}
