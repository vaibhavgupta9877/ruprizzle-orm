//! Runtime errors.

/// Errors that can be returned by ruprizzle operations.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("operation not yet implemented")]
    NotImplemented,

    #[error("{0}")]
    Message(String),
}
