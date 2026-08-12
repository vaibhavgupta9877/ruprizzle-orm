//! Migration directory scanning, checksum verification, and application.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ruprizzle::{Executor, Pool, RowBatch, Tx, Value};
use ruprizzle_core::ir::Provider;
use ruprizzle_dialect::DbDialect;
use sha2::{Digest, Sha256};
use sqlx::Row;

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
    pub async fn ensure_table(&self, pool: &Pool) -> Result<(), Error> {
        let sql = "CREATE TABLE IF NOT EXISTS _ruprizzle_migrations (
            id TEXT PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            execution_ms BIGINT NOT NULL DEFAULT 0,
            rolled_back_at TIMESTAMPTZ
        )";

        if let Err(e) = pool
            .execute_raw(Cow::Owned(sql.to_owned()), Vec::new())
            .await
        {
            if tracking_table_exists(pool).await {
                return Ok(());
            }
            return Err(Error::from(e));
        }

        Ok(())
    }

    /// Returns the IDs of migrations that have not yet been applied.
    pub async fn pending(&self, pool: &Pool) -> Result<Vec<String>, Error> {
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
    pub async fn status(&self, pool: &Pool) -> Result<Status, Error> {
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
    pub async fn verify_checksums(&self, pool: &Pool) -> Result<(), Error> {
        self.ensure_table(pool).await?;

        let batch = pool
            .fetch_all_raw(
                Cow::Owned(
                    "SELECT id, checksum FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL"
                        .into(),
                ),
                Vec::new(),
            )
            .await?;
        let rows = decode_pair(batch)?;

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
    pub async fn apply_all(&self, pool: &Pool, accept_data_loss: bool) -> Result<Report, Error> {
        self.ensure_table(pool).await?;
        self.verify_checksums(pool).await?;

        let is_postgres = pool.provider() == Provider::Postgres;
        let dialect = pool.dialect();

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

            let tx = Tx::begin(pool).await?;

            if is_postgres {
                tx.execute_raw(
                    Cow::Owned("SELECT pg_advisory_xact_lock($1)".into()),
                    vec![Value::I64(advisory_lock_key())],
                )
                .await?;
            }

            // Re-read inside the lock. Our pending set was computed before the
            // lock was held, so a concurrent deployer may have applied this
            // migration in between; re-running its DDL would fail on
            // "already exists" for what is really a no-op.
            let already_sql = format!(
                "SELECT id FROM _ruprizzle_migrations \
                 WHERE id = {} AND rolled_back_at IS NULL",
                dialect.placeholder(0)
            );
            let already = tx
                .fetch_all_raw(
                    Cow::Owned(already_sql),
                    vec![Value::Str(m.id.clone().into())],
                )
                .await?;
            if !already.is_empty() {
                tx.rollback().await?;
                continue;
            }

            let stmt_start = Instant::now();

            let statements = split_statements(&m.up);
            tracing::info!(
                target: "ruprizzle::migrate",
                migration = %m.id,
                statements = statements.len(),
                "applying migration"
            );
            for (idx, stmt) in statements.iter().enumerate() {
                let sql = stmt.trim();
                if sql.is_empty() {
                    continue;
                }

                if sql.starts_with("-- ") || sql.starts_with("/*") {
                    continue;
                }

                if let Err(e) = tx.execute_raw(Cow::Owned(sql.to_owned()), Vec::new()).await {
                    return Err(Error::StatementFailed {
                        id: m.id,
                        line: idx + 1,
                        message: e.to_string(),
                    });
                }
            }

            let elapsed = stmt_start.elapsed().as_millis() as i64;
            let tracking_sql = format!(
                "INSERT INTO _ruprizzle_migrations (id, checksum, applied_at, execution_ms) \
                 VALUES ({}, {}, CURRENT_TIMESTAMP, {}) \
                 ON CONFLICT (id) DO UPDATE SET \
                   checksum = EXCLUDED.checksum, \
                   applied_at = CURRENT_TIMESTAMP, \
                   rolled_back_at = NULL, \
                   execution_ms = EXCLUDED.execution_ms",
                dialect.placeholder(0),
                dialect.placeholder(1),
                dialect.placeholder(2)
            );
            tx.execute_raw(
                Cow::Owned(tracking_sql),
                vec![
                    Value::Str(m.id.clone().into()),
                    Value::Str(m.meta.checksum.clone().into()),
                    Value::I64(elapsed),
                ],
            )
            .await?;

            tx.commit().await?;
            tracing::info!(
                target: "ruprizzle::migrate",
                migration = %m.id,
                elapsed_ms = elapsed,
                "migration applied"
            );
            applied_ids.push(m.id);
        }

        Ok(Report {
            applied: applied_ids,
            duration: start.elapsed(),
        })
    }

    /// Rolls back the last `n` applied migrations using their `down.sql` files.
    pub async fn rollback(&self, pool: &Pool, n: usize) -> Result<Report, Error> {
        self.ensure_table(pool).await?;

        let dialect = pool.dialect();
        let sql = format!(
            "SELECT id FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL ORDER BY id DESC LIMIT {}",
            dialect.placeholder(0)
        );
        let batch = pool
            .fetch_all_raw(Cow::Owned(sql), vec![Value::I64(n as i64)])
            .await?;
        let rows = decode_string_rows(batch)?;

        let mut applied = Vec::new();

        for id in rows {
            let m = self
                .migrations()?
                .into_iter()
                .find(|m| m.id == id)
                .ok_or_else(|| Error::MissingUp { id: id.clone() })?;

            let statements = split_statements(&m.down);
            for stmt in statements {
                let sql = stmt.trim();
                if !sql.is_empty() && !sql.starts_with("-- ") {
                    pool.execute_raw(Cow::Owned(sql.to_owned()), Vec::new())
                        .await?;
                }
            }

            let update_sql = format!(
                "UPDATE _ruprizzle_migrations SET rolled_back_at = CURRENT_TIMESTAMP WHERE id = {}",
                dialect.placeholder(0)
            );
            pool.execute_raw(Cow::Owned(update_sql), vec![Value::Str(id.as_str().into())])
                .await?;

            applied.push(id);
        }

        Ok(Report {
            applied,
            duration: Duration::ZERO,
        })
    }

    /// Records a migration as applied without executing its `up.sql`.
    pub async fn resolve(&self, pool: &Pool, id: &str) -> Result<(), Error> {
        self.ensure_table(pool).await?;

        let m = self
            .migrations()?
            .into_iter()
            .find(|m| m.id == id)
            .ok_or_else(|| Error::MissingUp { id: id.to_owned() })?;

        let checksum = compute_checksum(&m.up);
        let dialect = pool.dialect();
        let sql = format!(
            "INSERT INTO _ruprizzle_migrations (id, checksum, applied_at, execution_ms) \
             VALUES ({}, {}, CURRENT_TIMESTAMP, 0) \
             ON CONFLICT (id) DO UPDATE SET \
               checksum = EXCLUDED.checksum, \
               applied_at = CURRENT_TIMESTAMP, \
               rolled_back_at = NULL, \
               execution_ms = EXCLUDED.execution_ms",
            dialect.placeholder(0),
            dialect.placeholder(1)
        );
        pool.execute_raw(
            Cow::Owned(sql),
            vec![Value::Str(id.into()), Value::Str(checksum.into())],
        )
        .await?;

        Ok(())
    }

    /// Drops all user tables (except `_ruprizzle_migrations`) and clears the
    /// migration tracking table so the full migration history can be replayed.
    pub async fn reset(&self, pool: &Pool, dialect: &dyn DbDialect) -> Result<(), Error> {
        self.ensure_table(pool).await?;

        let tables = user_tables(pool).await?;
        if tables.is_empty() {
            pool.execute_raw(
                Cow::Owned("DELETE FROM _ruprizzle_migrations".into()),
                Vec::new(),
            )
            .await?;
            return Ok(());
        }

        if pool.provider() == Provider::Sqlite {
            let tx = Tx::begin(pool).await?;
            tx.execute_raw(Cow::Owned("PRAGMA foreign_keys = OFF".into()), Vec::new())
                .await?;
            for table in &tables {
                let sql = format!("DROP TABLE {};", dialect.quote_ident(table));
                tx.execute_raw(Cow::Owned(sql), Vec::new()).await?;
            }
            tx.execute_raw(Cow::Owned("PRAGMA foreign_keys = ON".into()), Vec::new())
                .await?;
            tx.commit().await?;
        } else {
            let tx = Tx::begin(pool).await?;
            for table in &tables {
                let sql = format!("DROP TABLE {} CASCADE;", dialect.quote_ident(table));
                tx.execute_raw(Cow::Owned(sql), Vec::new()).await?;
            }
            tx.commit().await?;
        }

        pool.execute_raw(
            Cow::Owned("DELETE FROM _ruprizzle_migrations".into()),
            Vec::new(),
        )
        .await?;

        Ok(())
    }

    async fn applied_ids(&self, pool: &Pool) -> Result<HashSet<String>, Error> {
        let batch = pool
            .fetch_all_raw(
                Cow::Owned(
                    "SELECT id FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL".into(),
                ),
                Vec::new(),
            )
            .await?;
        let rows = decode_string_rows(batch)?;
        Ok(rows.into_iter().collect())
    }
}

/// Whether the tracking table is queryable, used to tell a lost `CREATE TABLE`
/// race apart from a genuine failure.
async fn tracking_table_exists(pool: &Pool) -> bool {
    pool.execute_raw(
        Cow::Owned("SELECT 1 FROM _ruprizzle_migrations WHERE 1 = 0".into()),
        Vec::new(),
    )
    .await
    .is_ok()
}

async fn user_tables(pool: &Pool) -> Result<Vec<String>, Error> {
    let sql = if pool.provider() == Provider::Sqlite {
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' \
           AND name NOT LIKE 'sqlite_%' \
           AND name != '_ruprizzle_migrations'"
            .to_owned()
    } else {
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = current_schema() \
           AND table_type = 'BASE TABLE' \
           AND table_name != '_ruprizzle_migrations'"
            .to_owned()
    };

    let batch = pool.fetch_all_raw(Cow::Owned(sql), Vec::new()).await?;
    decode_string_rows(batch)
}

fn decode_string_rows(batch: RowBatch) -> Result<Vec<String>, Error> {
    match batch {
        RowBatch::Any(rows) => rows
            .iter()
            .map(|r| Ok(r.try_get::<String, _>(0)?))
            .collect(),
        RowBatch::Postgres(rows) => rows
            .iter()
            .map(|r| Ok(r.try_get::<String, _>(0)?))
            .collect(),
        RowBatch::Sqlite(rows) => rows
            .iter()
            .map(|r| Ok(r.try_get::<String, _>(0)?))
            .collect(),
        #[cfg(feature = "sqlite-rusqlite")]
        RowBatch::Rusqlite(rows) => rows.iter().map(|r| Ok(r.get::<String>(0)?)).collect(),
        _ => Err(Error::Message("unsupported row batch".into())),
    }
}

fn decode_pair(batch: RowBatch) -> Result<Vec<(String, String)>, Error> {
    match batch {
        RowBatch::Any(rows) => rows
            .iter()
            .map(|r| Ok((r.try_get::<String, _>(0)?, r.try_get::<String, _>(1)?)))
            .collect(),
        RowBatch::Postgres(rows) => rows
            .iter()
            .map(|r| Ok((r.try_get::<String, _>(0)?, r.try_get::<String, _>(1)?)))
            .collect(),
        RowBatch::Sqlite(rows) => rows
            .iter()
            .map(|r| Ok((r.try_get::<String, _>(0)?, r.try_get::<String, _>(1)?)))
            .collect(),
        #[cfg(feature = "sqlite-rusqlite")]
        RowBatch::Rusqlite(rows) => rows
            .iter()
            .map(|r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)))
            .collect(),
        _ => Err(Error::Message("unsupported row batch".into())),
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
