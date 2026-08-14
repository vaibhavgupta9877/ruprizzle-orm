//! Lightweight drift detection: compare the live database against a snapshot.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use ruprizzle::{Executor, Pool, RowBatch, Value};
use ruprizzle_core::ir::{Provider, Schema};
use sqlx::Row;

use crate::Error;

/// Detects drift between the live database and the given `schema`.
///
/// Returns a list of human-readable differences.  An empty list means the
/// database matches the snapshot at the level of tables, columns, and
/// nullability checked here.
pub async fn detect(pool: &Pool, schema: &Schema) -> Result<Vec<String>, Error> {
    let db_tables = match pool.provider() {
        Provider::Sqlite => sqlite_tables(pool).await?,
        Provider::Postgres => postgres_tables(pool).await?,
        Provider::Mysql => mysql_tables(pool).await?,
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

async fn sqlite_tables(pool: &Pool) -> Result<TableMap, Error> {
    let names_sql = "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_ruprizzle_migrations'";
    let batch = pool
        .fetch_all_raw(Cow::Owned(names_sql.into()), Vec::new())
        .await?;
    let names = decode_string_rows(batch)?;

    let mut out = TableMap::new();
    for name in names {
        let sql = format!("PRAGMA table_info({name})");
        let rows = pool.fetch_all_raw(Cow::Owned(sql), Vec::new()).await?;
        let mut cols = ColumnMap::new();
        for (col, notnull) in decode_sqlite_columns(rows)? {
            // INTEGER PRIMARY KEY in SQLite may report notnull=0 even though the
            // primary key constraint makes it non-nullable.
            cols.insert(col, notnull);
        }
        out.insert(name, cols);
    }

    Ok(out)
}

async fn mysql_tables(pool: &Pool) -> Result<TableMap, Error> {
    let names_sql = "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' AND table_name != '_ruprizzle_migrations'";
    let batch = pool
        .fetch_all_raw(Cow::Owned(names_sql.into()), Vec::new())
        .await?;
    let names = decode_string_rows(batch)?;

    let mut out = TableMap::new();
    for name in names {
        let sql = "SELECT column_name, is_nullable FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ?";
        let rows = pool
            .fetch_all_raw(
                Cow::Owned(sql.to_owned()),
                vec![Value::Str(name.as_str().into())],
            )
            .await?;
        let mut cols = ColumnMap::new();
        for (col, nullable) in decode_pair(rows)? {
            cols.insert(col, nullable == "NO");
        }
        out.insert(name, cols);
    }
    Ok(out)
}

async fn postgres_tables(pool: &Pool) -> Result<TableMap, Error> {
    let names_sql = "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' AND table_name != '_ruprizzle_migrations'";
    let batch = pool
        .fetch_all_raw(Cow::Owned(names_sql.into()), Vec::new())
        .await?;
    let names = decode_string_rows(batch)?;

    let dialect = pool.dialect();
    let mut out = TableMap::new();
    for name in names {
        let sql = format!(
            "SELECT column_name::text, is_nullable \
             FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = {}",
            dialect.placeholder(0)
        );
        let rows = pool
            .fetch_all_raw(Cow::Owned(sql), vec![Value::Str(name.as_str().into())])
            .await?;
        let mut cols = ColumnMap::new();
        for (col, nullable) in decode_pair(rows)? {
            cols.insert(col, nullable == "NO");
        }
        out.insert(name, cols);
    }

    Ok(out)
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
        RowBatch::Mysql(rows) => rows
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
        RowBatch::Mysql(rows) => rows
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

fn decode_sqlite_columns(batch: RowBatch) -> Result<Vec<(String, bool)>, Error> {
    // PRAGMA table_info result columns are:
    // 0 cid, 1 name, 2 type, 3 notnull, 4 dflt_value, 5 pk.
    match batch {
        RowBatch::Any(rows) => rows
            .iter()
            .map(|r| {
                let name: String = r.try_get::<String, _>(1)?;
                let notnull: i64 = r.try_get::<i64, _>(3)?;
                let pk: i64 = r.try_get::<i64, _>(5)?;
                Ok((name, notnull != 0 || pk != 0))
            })
            .collect(),
        RowBatch::Postgres(rows) => rows
            .iter()
            .map(|r| {
                let name: String = r.try_get::<String, _>(1)?;
                let notnull: i64 = r.try_get::<i64, _>(3)?;
                let pk: i64 = r.try_get::<i64, _>(5)?;
                Ok((name, notnull != 0 || pk != 0))
            })
            .collect(),
        RowBatch::Sqlite(rows) => rows
            .iter()
            .map(|r| {
                let name: String = r.try_get::<String, _>(1)?;
                let notnull: i64 = r.try_get::<i64, _>(3)?;
                let pk: i64 = r.try_get::<i64, _>(5)?;
                Ok((name, notnull != 0 || pk != 0))
            })
            .collect(),
        RowBatch::Mysql(rows) => rows
            .iter()
            .map(|r| {
                let name: String = r.try_get::<String, _>(1)?;
                let notnull: i64 = r.try_get::<i64, _>(3)?;
                let pk: i64 = r.try_get::<i64, _>(5)?;
                Ok((name, notnull != 0 || pk != 0))
            })
            .collect(),
        #[cfg(feature = "sqlite-rusqlite")]
        RowBatch::Rusqlite(rows) => rows
            .iter()
            .map(|r| {
                let name = r.get::<String>(1)?;
                let notnull = r.get::<i64>(3)?;
                let pk = r.get::<i64>(5)?;
                Ok((name, notnull != 0 || pk != 0))
            })
            .collect(),
        _ => Err(Error::Message("unsupported row batch".into())),
    }
}
