//! Shared application state handed to every handler.

use std::sync::{Arc, Mutex, MutexGuard};

use op_core::{Clock, IdGen};

use crate::accounts::Accounts;
use crate::config::Config;
use crate::db::Db;
use crate::hub::Hub;

pub struct AppState {
    db: Mutex<Db>,
    ids: Mutex<IdGen>,
    pub accounts: Accounts,
    pub hub: Hub,
    pub config: Arc<Config>,
    pub clock: Arc<dyn Clock + Send + Sync>,
    /// Set once at start-up when the admin configured hooks.
    pub hooks: std::sync::OnceLock<Arc<crate::hooks::HookRunner>>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    /// Fails only if the database cannot be read to find where the server's id sequence left off.
    pub fn new(
        db: Db,
        accounts: Accounts,
        config: Config,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Shared, crate::db::DbError> {
        let ids = db.id_gen()?;
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            ids: Mutex::new(ids),
            accounts,
            hub: Hub::default(),
            config: Arc::new(config),
            clock,
            hooks: std::sync::OnceLock::new(),
        }))
    }

    /// The generator for ids the server itself writes (timer states, sessions it records). It stays strictly
    /// increasing for the life of the process and, through the database, across restarts.
    pub fn ids(&self) -> MutexGuard<'_, IdGen> {
        self.ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The database, one caller at a time. A panic elsewhere must not lock everyone out, so a poisoned lock is
    /// recovered: every write is a single transaction, so the data is never left half-written.
    pub fn db(&self) -> MutexGuard<'_, Db> {
        self.db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }
}
