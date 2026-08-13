//! Runtime errors.

/// Errors that can be returned by ruprizzle operations.
///
/// `#[non_exhaustive]`: new database backends and new constraint classes will
/// add variants, and that must not be a breaking change. Match with a trailing
/// `_ =>` arm.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Error {
    #[error("unique constraint violated on `{table}.{columns}`")]
    UniqueViolation {
        table: String,
        columns: String,
        value: Option<String>,
    },

    #[error("foreign key constraint violated on `{table}.{columns}`")]
    ForeignKeyViolation { table: String, columns: String },

    #[error("NOT NULL constraint violated on `{table}.{column}`")]
    NotNullViolation { table: String, column: String },

    #[error("check constraint violated on `{table}.{column}`")]
    CheckViolation { table: String, column: String },

    #[error("deadlock detected")]
    Deadlock,

    #[error("serialization failure")]
    SerializationFailure,

    #[error("connection failed: {reason}")]
    ConnectionFailure { reason: String },

    /// Every connection in the pool is checked out.
    ///
    /// `backend` is a static driver name rather than a `String` because this is
    /// raised on the acquire path of every query, and the caller is expected to
    /// match on the variant rather than read the text.
    #[error("{backend} connection pool exhausted")]
    PoolExhausted { backend: &'static str },

    #[error("connection acquire timed out: {reason}")]
    AcquireTimeout { reason: String },

    #[error("sqlx error: {0}")]
    Sqlx(sqlx::Error),

    #[cfg(feature = "postgres-tokio-postgres")]
    #[error("tokio-postgres error: {0}")]
    TokioPostgres(tokio_postgres::Error),

    #[error("operation not yet implemented")]
    NotImplemented,

    #[error("{0}")]
    Message(String),
}

impl Error {
    /// Returns a stable, non-sensitive category for telemetry.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::UniqueViolation { .. } => "unique_violation",
            Self::ForeignKeyViolation { .. } => "foreign_key_violation",
            Self::NotNullViolation { .. } => "not_null_violation",
            Self::CheckViolation { .. } => "check_violation",
            Self::Deadlock => "deadlock",
            Self::SerializationFailure => "serialization_failure",
            Self::ConnectionFailure { .. } => "connection_failure",
            Self::PoolExhausted { .. } => "pool_exhausted",
            Self::AcquireTimeout { .. } => "acquire_timeout",
            Self::Sqlx(_) => "sqlx",
            #[cfg(feature = "postgres-tokio-postgres")]
            Self::TokioPostgres(_) => "tokio_postgres",
            Self::NotImplemented => "not_implemented",
            Self::Message(_) => "message",
        }
    }
}

impl Error {
    /// Returns the captured value that violated a unique constraint, if any.
    ///
    /// This is user data and is intentionally not part of [`std::fmt::Display`].
    #[must_use]
    pub fn conflicting_value(&self) -> Option<&str> {
        match self {
            Self::UniqueViolation { value, .. } => value.as_deref(),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        classify_sqlx(err)
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl From<tokio_postgres::Error> for Error {
    fn from(err: tokio_postgres::Error) -> Self {
        classify_tokio_postgres(err)
    }
}

fn classify_sqlx(err: sqlx::Error) -> Error {
    use sqlx::Error as SqlxError;

    match err {
        SqlxError::Database(db_err) => {
            let msg = db_err.message().to_owned();
            let code = db_err.code().map(|c| c.to_string());
            let original = SqlxError::Database(db_err);

            if matches!(&code, Some(c) if is_connection_code(c)) {
                return Error::ConnectionFailure { reason: msg };
            }

            // Postgres SQLSTATE class 40 is transaction rollback; 40P01 is deadlock.
            if matches!(&code, Some(c) if c == "40P01") {
                return Error::Deadlock;
            }
            if matches!(&code, Some(c) if c == "40001") {
                return Error::SerializationFailure;
            }

            // Postgres specific codes.
            if matches!(&code, Some(c) if c == "23505") {
                return parse_unique_violation(&msg);
            }
            if matches!(&code, Some(c) if c == "23503") {
                return parse_foreign_key_violation(&msg);
            }
            if matches!(&code, Some(c) if c == "23502") {
                return parse_not_null_violation(&msg);
            }
            if matches!(&code, Some(c) if c == "23514" || c == "23507") {
                return parse_check_violation(&msg);
            }

            // SQLite uses one-letter class + five-digit codes; 5xx is constraint.
            if let Some(ref c) = code {
                if let Some(v) = sqlite_constraint_kind(c) {
                    if let Some(err) = parse_sqlite_constraint(&msg, v) {
                        return err;
                    }
                }
                if c == "08" || c.starts_with("08") {
                    return Error::ConnectionFailure { reason: msg };
                }
            }

            Error::Sqlx(original)
        }
        SqlxError::PoolTimedOut => Error::AcquireTimeout {
            reason: err.to_string(),
        },
        SqlxError::PoolClosed | SqlxError::Io(_) => {
            Error::ConnectionFailure {
                reason: err.to_string(),
            }
        }
        _ => Error::Sqlx(err),
    }
}

fn is_connection_code(code: &str) -> bool {
    code.starts_with("08")
}

fn sqlite_constraint_kind(code: &str) -> Option<&'static str> {
    match code {
        "2067" | "1555" => Some("UNIQUE"),
        "787" => Some("FOREIGN KEY"),
        "1299" => Some("NOT NULL"),
        "275" => Some("CHECK"),
        _ => None,
    }
}

fn parse_sqlite_constraint(msg: &str, kind: &'static str) -> Option<Error> {
    Some(match kind {
        "UNIQUE" => parse_sqlite_unique(msg),
        "FOREIGN KEY" => parse_sqlite_foreign_key(msg),
        "NOT NULL" => parse_sqlite_not_null(msg),
        "CHECK" => parse_sqlite_check(msg),
        _ => return None,
    })
}

fn parse_unique_violation(msg: &str) -> Error {
    // Postgres detail: "Key (email)=(a@b.c) already exists."
    let (table, columns, value) = parse_postgres_key_detail(msg);
    Error::UniqueViolation {
        table,
        columns,
        value,
    }
}

fn parse_foreign_key_violation(msg: &str) -> Error {
    let (table, columns) = parse_postgres_table_column(msg);
    Error::ForeignKeyViolation { table, columns }
}

fn parse_not_null_violation(msg: &str) -> Error {
    let (table, column) = parse_postgres_table_column_single(msg);
    Error::NotNullViolation { table, column }
}

fn parse_check_violation(msg: &str) -> Error {
    let (table, column) = parse_postgres_table_column_single(msg);
    Error::CheckViolation { table, column }
}

fn parse_sqlite_unique(msg: &str) -> Error {
    let (table, columns) = parse_sqlite_constraint_columns(msg, "UNIQUE constraint failed: ");
    Error::UniqueViolation {
        table,
        columns,
        value: None,
    }
}

fn parse_sqlite_foreign_key(msg: &str) -> Error {
    let (table, columns) = parse_sqlite_constraint_columns(msg, "FOREIGN KEY constraint failed: ");
    Error::ForeignKeyViolation { table, columns }
}

fn parse_sqlite_not_null(msg: &str) -> Error {
    let (table, column) = parse_sqlite_table_column(msg, "NOT NULL constraint failed: ");
    Error::NotNullViolation { table, column }
}

fn parse_sqlite_check(msg: &str) -> Error {
    let (table, column) = parse_sqlite_table_column(msg, "CHECK constraint failed: ");
    Error::CheckViolation { table, column }
}

fn parse_postgres_key_detail(msg: &str) -> (String, String, Option<String>) {
    if let Some(start) = msg.find("Key (") {
        let rest = &msg[start + 5..];
        if let Some(close) = rest.find(")=(") {
            let columns = &rest[..close];
            let rest2 = &rest[close + 3..];
            if let Some(end) = rest2.find(")") {
                let value = &rest2[..end];
                return (String::new(), columns.to_owned(), Some(value.to_owned()));
            }
        }
    }
    (String::new(), String::new(), None)
}

fn parse_postgres_table_column(msg: &str) -> (String, String) {
    if let Some(start) = msg.find(r#"table ""#) {
        let rest = &msg[start + 7..];
        if let Some(end) = rest.find(r#"""#) {
            let table = &rest[..end];
            return (table.to_owned(), String::new());
        }
    }
    (String::new(), String::new())
}

fn parse_postgres_table_column_single(msg: &str) -> (String, String) {
    if let Some(start) = msg.find("column \"") {
        let rest = &msg[start + 8..];
        if let Some(end) = rest.find("\"") {
            let full = &rest[..end];
            if let Some(dot) = full.rfind('.') {
                return (full[..dot].to_owned(), full[dot + 1..].to_owned());
            }
            return (String::new(), full.to_owned());
        }
    }
    (String::new(), String::new())
}

fn parse_sqlite_constraint_columns(msg: &str, prefix: &str) -> (String, String) {
    if let Some(rest) = msg.strip_prefix(prefix) {
        let full = rest.trim();
        if let Some(dot) = full.rfind('.') {
            return (full[..dot].to_owned(), full[dot + 1..].to_owned());
        }
    }
    (String::new(), String::new())
}

fn parse_sqlite_table_column(msg: &str, prefix: &str) -> (String, String) {
    parse_sqlite_constraint_columns(msg, prefix)
}

#[cfg(feature = "postgres-tokio-postgres")]
fn classify_tokio_postgres(err: tokio_postgres::Error) -> Error {
    if let Some(db) = err.as_db_error() {
        let msg = db.message().to_owned();
        let code = db.code().code();

        if is_connection_code(code) {
            return Error::ConnectionFailure { reason: msg };
        }

        // Postgres SQLSTATE class 40 is transaction rollback; 40P01 is deadlock.
        if code == "40P01" {
            return Error::Deadlock;
        }
        if code == "40001" {
            return Error::SerializationFailure;
        }

        // Postgres specific codes.
        if code == "23505" {
            return parse_unique_violation(&msg);
        }
        if code == "23503" {
            return parse_foreign_key_violation(&msg);
        }
        if code == "23502" {
            return parse_not_null_violation(&msg);
        }
        if code == "23514" || code == "23507" {
            return parse_check_violation(&msg);
        }

        return Error::TokioPostgres(err);
    }

    if err.is_closed() {
        return Error::ConnectionFailure {
            reason: err.to_string(),
        };
    }

    Error::TokioPostgres(err)
}
