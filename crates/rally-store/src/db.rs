use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::StoreError;

/// Current schema version. Bump when adding migrations.
const SCHEMA_VERSION: u32 = 1;

const MIGRATION_V1: &str = "
CREATE TABLE IF NOT EXISTS workspaces (
    id          TEXT    NOT NULL PRIMARY KEY,
    name        TEXT    NOT NULL,
    repo        TEXT,
    created_at  INTEGER NOT NULL,
    archived    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS agents (
    id              TEXT    NOT NULL PRIMARY KEY,
    workspace_id    TEXT    NOT NULL REFERENCES workspaces(id),
    role            TEXT    NOT NULL,
    runtime         TEXT    NOT NULL,
    state           TEXT    NOT NULL,
    pane_session    TEXT,
    pane_tab_index  INTEGER,
    pane_id         INTEGER,
    restart_count   INTEGER NOT NULL DEFAULT 0,
    metadata_json   TEXT    NOT NULL DEFAULT '{}',
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS events (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT    NOT NULL,
    kind         TEXT    NOT NULL,
    payload_json TEXT    NOT NULL,
    at_ms        INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS events_workspace ON events(workspace_id);
";

/// Thread-safe handle to the SQLite store.
///
/// Wraps a single `Connection` behind a mutex — the daemon uses a dedicated
/// writer task so there is never write contention at runtime.
#[derive(Clone)]
pub struct Store {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl Store {
    /// Execute a raw SQL query returning a single i64 — for integration tests only.
    pub fn raw_count(&self, sql: &str) -> i64 {
        self.conn.lock().unwrap().query_row(sql, [], |r| r.get(0)).unwrap_or(0)
    }

    /// Execute a raw SQL query returning ordered i64 values — for integration tests only.
    pub fn raw_i64_vec(&self, sql: &str) -> Vec<i64> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([], |r| r.get(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    /// Execute arbitrary SQL (for test-only transaction manipulation).
    pub fn raw_exec(&self, sql: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute_batch(sql);
    }

    /// Open (or create) the database at `path` with WAL mode enabled.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        configure(&conn)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an in-memory database — useful for tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        configure(&conn)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

fn configure(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous  = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    conn.execute_batch(MIGRATION_V1).map_err(|e| StoreError::Migration {
        version: 1,
        reason: e.to_string(),
    })?;

    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;

    Ok(())
}
