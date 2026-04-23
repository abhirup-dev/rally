use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("migration failed at version {version}: {reason}")]
    Migration { version: u32, reason: String },

    #[error("not found: {0}")]
    NotFound(String),
}
