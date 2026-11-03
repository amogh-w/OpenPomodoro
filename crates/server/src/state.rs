//! Shared application state handed to every handler.

use std::sync::{Arc, Mutex, MutexGuard};

use op_core::Clock;

use crate::accounts::Accounts;
use crate::config::Config;
use crate::db::Db;

pub struct AppState {
    db: Mutex<Db>,
    pub accounts: Accounts,
    pub config: Arc<Config>,
    pub clock: Arc<dyn Clock + Send + Sync>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn new(
        db: Db,
        accounts: Accounts,
        config: Config,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Shared {
        Arc::new(Self {
            db: Mutex::new(db),
            accounts,
            config: Arc::new(config),
            clock,
        })
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
