//! Migration directory scanning, checksum verification, and application.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ruprizzle_dialect::DbDialect;
use sha2::{Digest, Sha256};
use sqlx::AnyPool;

use crate::Error;

/// Metadata stored alongside each migration in `meta.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)]
pub struct MigrationMeta {
    pub id: String,
    pub checksum: String,
    pub destructive: bool,
    pub ruprizzle_version: String,
}

/// A migration loaded from disk.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Migration {
    pub id: String,
    pub up: String,
    pub down: String,
    pub meta: MigrationMeta,
}

/// Tracks which migrations are applied and which are pending.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Status {
    pub applied: Vec<String>,
    pub pending: Vec<String>,
}

/// Apply report.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Report {
    pub applied: Vec<String>,
    pub duration: Duration,
}

/// Migration engine.  Reads migrations from `dir` and applies them to `pool`.
#[derive(Debug, Clone)]
pub struct Migrator {
    dir: PathBuf,
}

impl Migrator {
    /// Creates a new migrator for the given directory.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Returns the migration directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Loads all migrations from disk, sorted by directory name.
    pub fn migrations(&self) -> Result<Vec<Migration>, Error> {
        if !self.dir.exists() {
            return Err(Error::DirectoryNotFound(self.dir.clone()));
        }

        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && path.join("up.sql").exists() {
                dirs.push(path);
            }
        }

        dirs.sort();

        let mut out = Vec::new();
        for dir in dirs {
            out.push(self.load_one(&dir)?);
        }
        Ok(out)
    }

    fn load_one(&self, dir: &Path) -> Result<Migration, Error> {
        let id = dir
            .file_name()
            .ok_or_else(|| Error::Io(std::io::Error::other("missing directory name")))?
            .to_string_lossy()
            .into_owned();

        let up = std::fs::read_to_string(dir.join("up.sql"))?;
        let down = std::fs::read_to_string(dir.join("down.sql")).unwrap_or_default();

        let meta: MigrationMeta = if dir.join("meta.json").exists() {
            serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json"))?)?
        } else {
            MigrationMeta {
                id: id.clone(),
                checksum: compute_checksum(&up),
                destructive: up.to_lowercase().contains("drop table")
                    || up.to_lowercase().contains("drop column"),
                ruprizzle_version: env!("CARGO_PKG_VERSION").to_owned(),
            }
        };

        Ok(Migration { id, up, down, meta })
    }

    /// Creates the `_ruprizzle_migrations` tracking table if it does not exist.
    pub async fn ensure_table(&self, pool: &AnyPool) -> Result<(), Error> {
        let created = sqlx::query(
            "CREATE TABLE IF NOT EXISTS _ruprizzle_migrations (
                id TEXT PRIMARY KEY,
                checksum TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                execution_ms BIGINT NOT NULL DEFAULT 0,
                rolled_back_at TIMESTAMPTZ
            )",
        )
        .execute(pool)
        .await;

        match created {
            Ok(_) => Ok(()),
            // `CREATE TABLE IF NOT EXISTS` is not race-safe on Postgres: two
            // sessions that pass the existence check together both insert into
            // `pg_type`, and the loser fails on a duplicate key rather than
            // becoming a no-op. This runs before the advisory lock is taken, so
            // concurrent deployers reach it first; treat "the table exists now"
            // as the success it is.
            Err(e) => {
                if tracking_table_exists(pool).await {
                    Ok(())
                } else {
                    Err(Error::from(e))
                }
            }
        }
    }

    /// Returns the IDs of migrations that have not yet been applied.
    pub async fn pending(&self, pool: &AnyPool) -> Result<Vec<String>, Error> {
        self.ensure_table(pool).await?;
        let applied = self.applied_ids(pool).await?;

        let mut out = Vec::new();
        for m in self.migrations()? {
            if !applied.contains(&m.id) {
                out.push(m.id);
            }
        }
        Ok(out)
    }

    /// Returns applied/pending status.
    pub async fn status(&self, pool: &AnyPool) -> Result<Status, Error> {
        self.ensure_table(pool).await?;
        let applied = self.applied_ids(pool).await?;
        let migrations = self.migrations()?;

        let applied_ids: Vec<String> = migrations
            .iter()
            .filter(|m| applied.contains(&m.id))
            .map(|m| m.id.clone())
            .collect();
        let pending: Vec<String> = migrations
            .iter()
            .filter(|m| !applied.contains(&m.id))
            .map(|m| m.id.clone())
            .collect();

        Ok(Status {
            applied: applied_ids,
            pending,
        })
    }

    /// Verifies that every applied migration file on disk still matches the
    /// checksum recorded in the tracking table.
    pub async fn verify_checksums(&self, pool: &AnyPool) -> Result<(), Error> {
        self.ensure_table(pool).await?;

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, checksum FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL",
        )
        .fetch_all(pool)
        .await?;

        let by_id: BTreeMap<String, Migration> = self
            .migrations()?
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect();

        for (id, recorded) in rows {
            if let Some(m) = by_id.get(&id) {
                let actual = compute_checksum(&m.up);
                if recorded != actual {
                    return Err(Error::ChecksumMismatch { id });
                }
            }
        }

        Ok(())
    }

    /// Applies all pending migrations in a single transaction each.
    ///
    /// Pass `accept_data_loss: true` to allow destructive migrations to run.
    pub async fn apply_all(&self, pool: &AnyPool, accept_data_loss: bool) -> Result<Report, Error> {
        self.ensure_table(pool).await?;
        self.verify_checksums(pool).await?;

        // Use a transaction-scoped advisory lock on Postgres to stop another
        // `apply_all` from running concurrently.  SQLite and other backends are
        // not handled here because they require different locking primitives.
        let is_postgres = pool.acquire().await?.backend_name() == "PostgreSQL";

        let applied = self.applied_ids(pool).await?;
        let pending: Vec<Migration> = self
            .migrations()?
            .into_iter()
            .filter(|m| !applied.contains(&m.id))
            .collect();

        if pending.is_empty() {
            return Ok(Report {
                applied: Vec::new(),
                duration: Duration::ZERO,
            });
        }

        let start = Instant::now();
        let mut applied_ids = Vec::new();

        for m in pending {
            if m.up.contains("RUPRIZZLE:BACKFILL") && m.up.contains("-- UPDATE") {
                return Err(Error::BackfillRequired { id: m.id });
            }

            if m.meta.destructive && !accept_data_loss {
                return Err(Error::DestructiveBlocked { id: m.id });
            }

            let mut tx = pool.begin().await?;

            if is_postgres {
                sqlx::query("SELECT pg_advisory_xact_lock($1)")
                    .bind(advisory_lock_key())
                    .execute(&mut *tx)
                    .await?;
            }

            // Re-read inside the lock. Our pending set was computed before the
            // lock was held, so a concurrent deployer may have applied this
            // migration in between; re-running its DDL would fail on
            // "already exists" for what is really a no-op.
            let already: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM _ruprizzle_migrations \
                 WHERE id = $1 AND rolled_back_at IS NULL",
            )
            .bind(&m.id)
            .fetch_optional(&mut *tx)
            .await?;
            if already.is_some() {
                tx.rollback().await?;
                continue;
            }

            let stmt_start = Instant::now();

            let statements = split_statements(&m.up);
            for (idx, stmt) in statements.iter().enumerate() {
                let sql = stmt.trim();
                if sql.is_empty() {
                    continue;
                }

                if sql.starts_with("-- ") || sql.starts_with("/*") {
                    continue;
                }

                if let Err(e) = sqlx::query(sql).execute(&mut *tx).await {
                    return Err(Error::StatementFailed {
                        id: m.id,
                        line: idx + 1,
                        message: e.to_string(),
                    });
                }
            }

            let elapsed = stmt_start.elapsed().as_millis() as i64;
            sqlx::query(
                "INSERT INTO _ruprizzle_migrations (id, checksum, applied_at, execution_ms)
                 VALUES ($1, $2, CURRENT_TIMESTAMP, $3)
                 ON CONFLICT (id) DO UPDATE SET
                   checksum = EXCLUDED.checksum,
                   applied_at = CURRENT_TIMESTAMP,
                   rolled_back_at = NULL,
                   execution_ms = EXCLUDED.execution_ms",
            )
            .bind(&m.id)
            .bind(&m.meta.checksum)
            .bind(elapsed)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
            applied_ids.push(m.id);
        }

        Ok(Report {
            applied: applied_ids,
            duration: start.elapsed(),
        })
    }

    /// Rolls back the last `n` applied migrations using their `down.sql` files.
    pub async fn rollback(&self, pool: &AnyPool, n: usize) -> Result<Report, Error> {
        self.ensure_table(pool).await?;

        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL ORDER BY id DESC LIMIT $1")
                .bind(n as i64)
                .fetch_all(pool)
                .await?;

        let mut applied = Vec::new();

        for (id,) in rows {
            let m = self
                .migrations()?
                .into_iter()
                .find(|m| m.id == id)
                .ok_or_else(|| Error::MissingUp { id: id.clone() })?;

            let statements = split_statements(&m.down);
            for stmt in statements {
                let sql = stmt.trim();
                if !sql.is_empty() && !sql.starts_with("-- ") {
                    sqlx::query(sql).execute(pool).await?;
                }
            }

            sqlx::query(
                "UPDATE _ruprizzle_migrations SET rolled_back_at = CURRENT_TIMESTAMP WHERE id = $1",
            )
            .bind(&id)
            .execute(pool)
            .await?;

            applied.push(id);
        }

        Ok(Report {
            applied,
            duration: Duration::ZERO,
        })
    }

    /// Records a migration as applied without executing its `up.sql`.
    pub async fn resolve(&self, pool: &AnyPool, id: &str) -> Result<(), Error> {
        self.ensure_table(pool).await?;

        let m = self
            .migrations()?
            .into_iter()
            .find(|m| m.id == id)
            .ok_or_else(|| Error::MissingUp { id: id.to_owned() })?;

        let checksum = compute_checksum(&m.up);
        sqlx::query(
            "INSERT INTO _ruprizzle_migrations (id, checksum, applied_at, execution_ms) \
             VALUES ($1, $2, CURRENT_TIMESTAMP, 0) \
             ON CONFLICT (id) DO UPDATE SET \
               checksum = EXCLUDED.checksum, \
               applied_at = CURRENT_TIMESTAMP, \
               rolled_back_at = NULL, \
               execution_ms = EXCLUDED.execution_ms",
        )
        .bind(id)
        .bind(&checksum)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Drops all user tables (except `_ruprizzle_migrations`) and clears the
    /// migration tracking table so the full migration history can be replayed.
    pub async fn reset(&self, pool: &AnyPool, dialect: &dyn DbDialect) -> Result<(), Error> {
        self.ensure_table(pool).await?;

        let tables = user_tables(pool).await?;
        if tables.is_empty() {
            sqlx::query("DELETE FROM _ruprizzle_migrations")
                .execute(pool)
                .await?;
            return Ok(());
        }

        let backend = pool.acquire().await?.backend_name().to_owned();

        if backend == "SQLite" {
            let mut conn = pool.acquire().await?;
            sqlx::query("PRAGMA foreign_keys = OFF")
                .execute(&mut *conn)
                .await?;
            for table in &tables {
                let sql = format!("DROP TABLE {};", dialect.quote_ident(table));
                sqlx::query(&sql).execute(&mut *conn).await?;
            }
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *conn)
                .await?;
        } else {
            let mut tx = pool.begin().await?;
            for table in &tables {
                let sql = format!("DROP TABLE {} CASCADE;", dialect.quote_ident(table));
                sqlx::query(&sql).execute(&mut *tx).await?;
            }
            tx.commit().await?;
        }

        sqlx::query("DELETE FROM _ruprizzle_migrations")
            .execute(pool)
            .await?;

        Ok(())
    }

    async fn applied_ids(&self, pool: &AnyPool) -> Result<HashSet<String>, Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL")
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

/// Whether the tracking table is queryable, used to tell a lost `CREATE TABLE`
/// race apart from a genuine failure.
async fn tracking_table_exists(pool: &AnyPool) -> bool {
    sqlx::query("SELECT 1 FROM _ruprizzle_migrations WHERE 1 = 0")
        .execute(pool)
        .await
        .is_ok()
}

async fn user_tables(pool: &AnyPool) -> Result<Vec<String>, Error> {
    let backend = pool.acquire().await?.backend_name().to_owned();

    if backend == "SQLite" {
        sqlx::query_scalar(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' \
               AND name NOT LIKE 'sqlite_%' \
               AND name != '_ruprizzle_migrations'",
        )
        .fetch_all(pool)
        .await
        .map_err(Error::from)
    } else {
        sqlx::query_scalar(
            "SELECT table_name::text FROM information_schema.tables \
             WHERE table_schema = current_schema() \
               AND table_type = 'BASE TABLE' \
               AND table_name != '_ruprizzle_migrations'",
        )
        .fetch_all(pool)
        .await
        .map_err(Error::from)
    }
}

/// Computes the SHA-256 checksum of a migration's `up.sql`.
pub fn compute_checksum(up: &str) -> String {
    let normalized = up.replace("\r\n", "\n");
    let digest = Sha256::digest(normalized.as_bytes());
    digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// The advisory lock key for migration application.
///
/// Derived from the tracking table name rather than a literal, because advisory
/// lock keys share one namespace per database: a hardcoded small integer will
/// eventually contend with an unrelated application that picked the same one.
fn advisory_lock_key() -> i64 {
    let digest = Sha256::digest(b"_ruprizzle_migrations");
    i64::from_be_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}

/// Splits SQL text into individual statements, respecting comments and literals.
pub fn split_statements(sql: &str) -> Vec<String> {
    // Simple SQL-aware splitter.  It ignores `;` inside `--` comments, `/* */
    // block comments, and single-quoted string literals, so generated markers and
    // down.sql notes do not break statement boundaries.  Comments are stripped
    // from the returned statements.
    let mut statements = Vec::new();
    let mut current = String::new();
    // Scanned as `char`s, not bytes: `u8 as char` is a Latin-1 widening, which
    // silently turns any multi-byte UTF-8 sequence into mojibake.
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '\'' => {
                current.push('\'');
                i += 1;
                while i < chars.len() {
                    current.push(chars[i]);
                    if chars[i] == '\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            // Dollar-quoted body (`$$ … $$` or `$tag$ … $tag$`): copied verbatim,
            // so a `;` or `--` inside a plpgsql function cannot split the statement.
            '$' if dollar_tag_len(&chars, i).is_some() => {
                let tag_len = dollar_tag_len(&chars, i).unwrap_or(0);
                let tag: Vec<char> = chars[i..i + tag_len].to_vec();
                current.extend(tag.iter());
                i += tag_len;
                while i < chars.len() {
                    if chars[i] == '$' && matches_at(&chars, i, &tag) {
                        current.extend(tag.iter());
                        i += tag_len;
                        break;
                    }
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '-' if i + 1 < chars.len() && chars[i + 1] == '-' => {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                // The newline is preserved to act as whitespace.
                current.push(' ');
                i += 1;
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i + 1 < chars.len() {
                    i += 2;
                }
                current.push(' ');
            }
            ';' => {
                if !current.trim().is_empty() {
                    statements.push(current.trim().to_owned());
                }
                current.clear();
                i += 1;
            }
            c => {
                current.push(c);
                i += 1;
            }
        }
    }

    if !current.trim().is_empty() {
        statements.push(current.trim().to_owned());
    }

    statements
}

/// If a dollar-quote tag (`$$` or `$name$`) starts at `i`, returns its length.
///
/// Returns `None` for a bind placeholder such as `$1`, because a Postgres tag
/// may not start with a digit.
fn dollar_tag_len(chars: &[char], i: usize) -> Option<usize> {
    if chars.get(i) != Some(&'$') {
        return None;
    }
    let mut j = i + 1;
    if chars.get(j).is_some_and(|c| *c != '$') {
        if !chars.get(j).is_some_and(|c| c.is_alphabetic() || *c == '_') {
            return None;
        }
        while chars
            .get(j)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
        {
            j += 1;
        }
    }
    if chars.get(j) == Some(&'$') {
        Some(j - i + 1)
    } else {
        None
    }
}

/// Whether `tag` occurs at `i` in `chars`.
fn matches_at(chars: &[char], i: usize, tag: &[char]) -> bool {
    chars.len() >= i + tag.len() && chars[i..i + tag.len()] == *tag
}

#[cfg(test)]
mod tests {
    use super::advisory_lock_key;

    #[test]
    fn lock_key_is_stable_and_not_a_small_literal() {
        let k = advisory_lock_key();
        assert_eq!(k, advisory_lock_key(), "key must be deterministic");
        assert!(
            k.unsigned_abs() > u64::from(u16::MAX),
            "key {k} is small enough to collide with a hand-picked literal"
        );
    }
}
