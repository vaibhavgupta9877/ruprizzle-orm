//! Lightweight drift detection: compare the live database against a snapshot.

use std::collections::{HashMap, HashSet};

use ruprizzle_core::ir::Schema;
use sqlx::{AnyPool, Row};

use crate::Error;

/// Detects drift between the live database and the given `schema`.
///
/// Returns a list of human-readable differences.  An empty list means the
/// database matches the snapshot at the level of tables, columns, and
/// nullability checked here.
pub async fn detect(pool: &AnyPool, schema: &Schema) -> Result<Vec<String>, Error> {
    let conn = pool.acquire().await?;
    let backend = conn.backend_name();

    let db_tables = if backend == "SQLite" {
        sqlite_tables(pool).await?
    } else {
        postgres_tables(pool).await?
    };

    let mut drift = Vec::new();

    let expected_tables: HashSet<&str> = schema.models.values().map(|m| m.table.as_str()).collect();

    let db_table_names: HashSet<&str> = db_tables.keys().map(std::string::String::as_str).collect();

    for table in db_table_names.difference(&expected_tables) {
        drift.push(format!(
            "table `{table}` exists in the database but not in the snapshot"
        ));
    }

    for table in expected_tables.difference(&db_table_names) {
        drift.push(format!("table `{table}` is missing from the database"));
    }

    for (table, db_cols) in &db_tables {
        let Some(model) = schema.models.values().find(|m| m.table == *table) else {
            continue;
        };

        let expected_cols: HashMap<&str, bool> = model
            .fields
            .values()
            .filter(|f| f.has_column())
            .map(|f| (f.column.as_str(), f.optional))
            .collect();

        let db_col_names: HashSet<&str> = db_cols.keys().map(std::string::String::as_str).collect();

        for col in db_col_names.difference(&expected_cols.keys().copied().collect()) {
            drift.push(format!(
                "table `{table}` has column `{col}` not present in the snapshot"
            ));
        }

        for col in expected_cols.keys().filter(|c| !db_col_names.contains(**c)) {
            drift.push(format!("table `{table}` is missing column `{col}`"));
        }

        for (col, optional) in &expected_cols {
            if let Some(db_notnull) = db_cols.get(*col) {
                let db_notnull = *db_notnull;
                if db_notnull && *optional {
                    drift.push(format!(
                        "table `{table}` column `{col}` is NOT NULL in the database but nullable in the snapshot"
                    ));
                } else if !db_notnull && !*optional {
                    drift.push(format!(
                        "table `{table}` column `{col}` is nullable in the database but NOT NULL in the snapshot"
                    ));
                }
            }
        }
    }

    Ok(drift)
}

type ColumnMap = HashMap<String, bool>;
type TableMap = HashMap<String, ColumnMap>;

async fn sqlite_tables(pool: &AnyPool) -> Result<TableMap, Error> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_ruprizzle_migrations'",
    )
    .fetch_all(pool)
    .await?;

    let mut out = TableMap::new();
    for name in names {
        let mut cols = ColumnMap::new();
        let rows = sqlx::query(&format!("PRAGMA table_info({name})"))
            .fetch_all(pool)
            .await?;
        for row in rows {
            let col: String = row.try_get("name")?;
            let notnull: i64 = row.try_get::<i64, _>("notnull")?;
            let pk: i64 = row.try_get::<i64, _>("pk")?;
            // INTEGER PRIMARY KEY in SQLite may report notnull=0 even though the
            // primary key constraint makes it non-nullable.
            cols.insert(col, notnull != 0 || pk != 0);
        }
        out.insert(name, cols);
    }

    Ok(out)
}

async fn postgres_tables(pool: &AnyPool) -> Result<TableMap, Error> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' AND table_name != '_ruprizzle_migrations'",
    )
    .fetch_all(pool)
    .await?;

    let mut out = TableMap::new();
    for name in names {
        let mut cols = ColumnMap::new();
        let rows = sqlx::query(
            "SELECT column_name::text, is_nullable \
             FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = $1",
        )
        .bind(&name)
        .fetch_all(pool)
        .await?;
        for row in rows {
            let col: String = row.try_get("column_name")?;
            let nullable: String = row.try_get("is_nullable")?;
            cols.insert(col, nullable == "NO");
        }
        out.insert(name, cols);
    }

    Ok(out)
}
