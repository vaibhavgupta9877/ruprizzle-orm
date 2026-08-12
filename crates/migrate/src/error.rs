//! Migration errors.

use std::path::PathBuf;

/// Errors from the migration engine.
///
/// `#[non_exhaustive]`: see the note on `ruprizzle::Error`.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("runtime error: {0}")]
    Runtime(#[from] ruprizzle::Error),

    #[error("checksum mismatch for migration {id}: file has changed since it was applied")]
    ChecksumMismatch { id: String },

    #[error("migration {id} is destructive; pass --accept-data-loss to proceed")]
    DestructiveBlocked { id: String },

    #[error("migration {id} contains no up.sql")]
    MissingUp { id: String },

    #[error(
        "migration {id} contains an unfilled RUPRIZZLE:BACKFILL block; edit the backfill and try again"
    )]
    BackfillRequired { id: String },

    #[error("migration {id} failed at statement {line}: {message}")]
    StatementFailed {
        id: String,
        line: usize,
        message: String,
    },

    #[error("migration directory not found: {0}")]
    DirectoryNotFound(PathBuf),

    #[error("{0}")]
    Message(String),
}
