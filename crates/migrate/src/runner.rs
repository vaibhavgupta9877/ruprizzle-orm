//! Migration directory scanning, checksum verification, and application.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _ruprizzle_migrations (
                id TEXT PRIMARY KEY,
                checksum TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                execution_ms BIGINT NOT NULL DEFAULT 0,
                rolled_back_at TIMESTAMPTZ
            )",
        )
        .execute(pool)
        .await?;
        Ok(())
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
            if m.meta.destructive && !accept_data_loss {
                return Err(Error::DestructiveBlocked { id: m.id });
            }

            let mut tx = pool.begin().await?;

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

            let elapsed = start.elapsed().as_millis() as i64;
            sqlx::query(
                "INSERT INTO _ruprizzle_migrations (id, checksum, applied_at, execution_ms)
                 VALUES ($1, $2, CURRENT_TIMESTAMP, $3)",
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

    async fn applied_ids(&self, pool: &AnyPool) -> Result<HashSet<String>, Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM _ruprizzle_migrations WHERE rolled_back_at IS NULL")
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

fn compute_checksum(up: &str) -> String {
    let normalized = up.replace("\r\n", "\n");
    let digest = Sha256::digest(normalized.as_bytes());
    digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

fn split_statements(sql: &str) -> Vec<&str> {
    // Split on semicolons.  This is intentionally simple: the migration file is
    // generated by this crate and each statement is on its own line.  It will not
    // correctly handle semicolons inside string literals, but generated migrations
    // avoid those.
    sql.split(';').collect()
}
