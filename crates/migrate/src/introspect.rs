//! Database introspection for `ruprizzle db pull`.
//!
//! The queries intentionally return textual metadata. This keeps one
//! introspector usable through `SQLx`'s `Any`, native `SQLx` pools, and the optional
//! rusqlite path without making the generated schema depend on driver types.

use std::borrow::Cow;
use std::collections::BTreeMap;

use ruprizzle::{Executor, Pool, RowBatch, Value};
use ruprizzle_core::ir::Provider;
use sqlx::{ColumnIndex, Decode, Row, Type};

use crate::Error;

/// A schema discovered from a live database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseSchema {
    /// The provider used for the connection.
    pub provider: Provider,
    /// Tables in database order.
    pub tables: Vec<Table>,
}

/// A discovered database table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Physical table name.
    pub name: String,
    /// Columns in ordinal order.
    pub columns: Vec<Column>,
    /// Primary-key columns in declaration order.
    pub primary_key: Vec<String>,
    /// Secondary indexes and unique constraints.
    pub indexes: Vec<Index>,
    /// Foreign-key constraints declared on this table.
    pub foreign_keys: Vec<ForeignKey>,
}

/// A discovered foreign-key constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    /// Physical constraint name, when the database exposes one.
    pub name: String,
    /// Referencing columns in declaration order.
    pub columns: Vec<String>,
    /// Referenced table.
    pub target_table: String,
    /// Referenced columns in declaration order.
    pub target_columns: Vec<String>,
    /// Database delete action, such as `CASCADE`.
    pub on_delete: Option<String>,
}

/// A discovered database column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Physical column name.
    pub name: String,
    /// Database-reported type string.
    pub db_type: String,
    /// Whether the column accepts `NULL`.
    pub nullable: bool,
    /// Database default expression, if present.
    pub default: Option<String>,
    /// Whether the database generates an increasing value.
    pub auto_increment: bool,
    /// Whether the column belongs to the primary key.
    pub primary_key: bool,
}

/// A discovered index or unique constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    /// Physical index or constraint name.
    pub name: String,
    /// Whether duplicate values are rejected.
    pub unique: bool,
    /// Indexed columns in index order.
    pub columns: Vec<String>,
}

/// Introspects the tables, columns, primary keys, and indexes visible to `pool`.
///
/// Foreign-key constraints are grouped by constraint name so the renderer can
/// emit paired relation fields instead of losing relational structure.
///
/// # Errors
///
/// Returns a migration error if metadata cannot be queried or decoded.
pub async fn pull(pool: &Pool) -> Result<DatabaseSchema, Error> {
    let provider = pool.provider();
    let names_sql = match provider {
        Provider::Sqlite => {
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_ruprizzle_migrations' ORDER BY name"
        }
        Provider::Postgres => {
            "SELECT table_name::text FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' AND table_name != '_ruprizzle_migrations' ORDER BY table_name"
        }
        Provider::Mysql => {
            "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' AND table_name != '_ruprizzle_migrations' ORDER BY table_name"
        }
    };

    let names = fetch_rows(pool, names_sql, Vec::new(), 1)
        .await?
        .into_iter()
        .filter_map(|row| row.first().cloned().flatten())
        .collect::<Vec<_>>();

    let mut tables = Vec::with_capacity(names.len());
    for name in names {
        tables.push(introspect_table(pool, provider, &name).await?);
    }

    Ok(DatabaseSchema { provider, tables })
}

async fn introspect_table(pool: &Pool, provider: Provider, name: &str) -> Result<Table, Error> {
    let mut columns = fetch_columns(pool, provider, name).await?;
    let primary_key = fetch_primary_key(pool, provider, name, &columns).await?;
    for column in &mut columns {
        column.primary_key = primary_key.iter().any(|key| key == &column.name);
    }
    let indexes = fetch_indexes(pool, provider, name, &primary_key).await?;
    let foreign_keys = fetch_foreign_keys(pool, provider, name).await?;

    Ok(Table {
        name: name.to_owned(),
        columns,
        primary_key,
        indexes,
        foreign_keys,
    })
}

async fn fetch_primary_key(
    pool: &Pool,
    provider: Provider,
    table: &str,
    columns: &[Column],
) -> Result<Vec<String>, Error> {
    if provider == Provider::Sqlite {
        return Ok(columns
            .iter()
            .filter(|column| column.primary_key)
            .map(|column| column.name.clone())
            .collect());
    }

    let rows = match provider {
        Provider::Postgres => fetch_rows(
            pool,
            "SELECT kcu.column_name::text, kcu.ordinal_position::text FROM information_schema.table_constraints tc JOIN information_schema.key_column_usage kcu ON kcu.constraint_name = tc.constraint_name AND kcu.table_schema = tc.table_schema AND kcu.table_name = tc.table_name WHERE tc.table_schema = current_schema() AND tc.table_name = $1 AND tc.constraint_type = 'PRIMARY KEY' ORDER BY kcu.ordinal_position",
            vec![Value::Str(table.to_owned().into())],
            2,
        )
        .await?,
        Provider::Mysql => fetch_rows(
            pool,
            "SELECT column_name, seq_in_index FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name = 'PRIMARY' ORDER BY seq_in_index",
            vec![Value::Str(table.to_owned().into())],
            2,
        )
        .await?,
        Provider::Sqlite => Vec::new(),
    };

    rows.into_iter().map(|row| required(&row, 0)).collect()
}

async fn fetch_columns(pool: &Pool, provider: Provider, table: &str) -> Result<Vec<Column>, Error> {
    let rows = match provider {
        Provider::Sqlite => {
            let sql = format!("PRAGMA table_info('{}')", escape_sql_literal(table));
            fetch_rows(pool, &sql, Vec::new(), 6).await?
        }
        Provider::Postgres => fetch_rows(
            pool,
            "SELECT column_name::text, data_type::text, udt_name::text, is_nullable::text, column_default::text, ordinal_position::text FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = $1 ORDER BY ordinal_position",
            vec![Value::Str(table.to_owned().into())],
            6,
        )
        .await?,
        Provider::Mysql => fetch_rows(
            pool,
            "SELECT column_name, column_type, is_nullable, column_default, extra, ordinal_position FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ordinal_position",
            vec![Value::Str(table.to_owned().into())],
            6,
        )
        .await?,
    };

    rows.into_iter()
        .map(|row| match provider {
            Provider::Sqlite => {
                let name = required(&row, 1)?;
                let db_type = required(&row, 2)?;
                let not_null = value(&row, 3).is_some_and(|v| v != "0");
                let default = value(&row, 4);
                let primary_key = value(&row, 5).is_some_and(|v| v != "0");
                let auto_increment = primary_key && db_type.eq_ignore_ascii_case("INTEGER");
                Ok(Column {
                    name,
                    db_type,
                    nullable: !not_null && !primary_key,
                    default,
                    auto_increment,
                    primary_key,
                })
            }
            Provider::Postgres => {
                let name = required(&row, 0)?;
                let data_type = required(&row, 1)?;
                let udt = value(&row, 2).unwrap_or_default();
                let db_type = if data_type == "USER-DEFINED" {
                    udt
                } else {
                    data_type
                };
                let default = value(&row, 4);
                let auto_increment = default
                    .as_deref()
                    .is_some_and(|v| v.to_ascii_lowercase().contains("nextval("));
                Ok(Column {
                    name,
                    db_type,
                    nullable: value(&row, 3).as_deref() == Some("YES"),
                    default,
                    auto_increment,
                    primary_key: false,
                })
            }
            Provider::Mysql => {
                let name = required(&row, 0)?;
                let extra = value(&row, 4).unwrap_or_default();
                Ok(Column {
                    name,
                    db_type: required(&row, 1)?,
                    nullable: value(&row, 2).as_deref() == Some("YES"),
                    default: value(&row, 3),
                    auto_increment: extra.to_ascii_lowercase().contains("auto_increment"),
                    primary_key: false,
                })
            }
        })
        .collect()
}

async fn fetch_indexes(
    pool: &Pool,
    provider: Provider,
    table: &str,
    primary_key: &[String],
) -> Result<Vec<Index>, Error> {
    let rows = match provider {
        Provider::Sqlite => {
            let sql = format!("PRAGMA index_list('{}')", escape_sql_literal(table));
            fetch_rows(pool, &sql, Vec::new(), 5).await?
        }
        Provider::Postgres => fetch_rows(
            pool,
            "SELECT index_name::text, (NOT non_unique)::text, column_name::text, ordinal_position::text FROM information_schema.statistics WHERE table_schema = current_schema() AND table_name = $1 ORDER BY index_name, ordinal_position",
            vec![Value::Str(table.to_owned().into())],
            4,
        )
        .await?,
        Provider::Mysql => fetch_rows(
            pool,
            "SELECT index_name, non_unique, column_name, seq_in_index FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? ORDER BY index_name, seq_in_index",
            vec![Value::Str(table.to_owned().into())],
            4,
        )
        .await?,
    };

    let mut grouped = BTreeMap::<String, Index>::new();
    for row in rows {
        let (name, unique, column) = match provider {
            Provider::Sqlite => (
                required(&row, 1)?,
                value(&row, 2).is_some_and(|v| v != "0"),
                None,
            ),
            Provider::Postgres => (
                required(&row, 0)?,
                value(&row, 1).is_some_and(|v| v == "true" || v == "1"),
                Some(required(&row, 2)?),
            ),
            Provider::Mysql => (
                required(&row, 0)?,
                value(&row, 1).is_some_and(|v| v == "0"),
                Some(required(&row, 2)?),
            ),
        };

        let entry = grouped.entry(name.clone()).or_insert_with(|| Index {
            name,
            unique,
            columns: Vec::new(),
        });
        if let Some(column) = column {
            entry.columns.push(column);
        } else {
            let sql = format!("PRAGMA index_info('{}')", escape_sql_literal(&entry.name));
            let info = fetch_rows(pool, &sql, Vec::new(), 3).await?;
            entry.columns = info
                .into_iter()
                .filter_map(|row| row.get(2).cloned().flatten())
                .collect();
        }
    }

    Ok(grouped
        .into_values()
        .filter(|index| index.name != "PRIMARY" && index.columns != primary_key)
        .filter_map(|mut index| {
            if index.name.starts_with("sqlite_autoindex_") {
                if !index.unique {
                    return None;
                }
                index.name = format!("{table}_{}_key", index.columns.join("_"));
            }
            Some(index)
        })
        .collect())
}

async fn fetch_foreign_keys(
    pool: &Pool,
    provider: Provider,
    table: &str,
) -> Result<Vec<ForeignKey>, Error> {
    let rows = match provider {
        Provider::Sqlite => {
            let sql = format!("PRAGMA foreign_key_list('{}')", escape_sql_literal(table));
            fetch_rows(pool, &sql, Vec::new(), 8).await?
        }
        Provider::Postgres => fetch_rows(
            pool,
            "SELECT constraint_name::text, column_name::text, referenced_table_name::text, referenced_column_name::text, ordinal_position::text FROM information_schema.key_column_usage WHERE table_schema = current_schema() AND table_name = $1 AND referenced_table_name IS NOT NULL ORDER BY constraint_name, ordinal_position",
            vec![Value::Str(table.to_owned().into())],
            5,
        )
        .await?,
        Provider::Mysql => fetch_rows(
            pool,
            "SELECT constraint_name, column_name, referenced_table_name, referenced_column_name, ordinal_position FROM information_schema.key_column_usage WHERE table_schema = DATABASE() AND table_name = ? AND referenced_table_name IS NOT NULL ORDER BY constraint_name, ordinal_position",
            vec![Value::Str(table.to_owned().into())],
            5,
        )
        .await?,
    };

    let mut grouped = BTreeMap::<String, ForeignKey>::new();
    for row in rows {
        let (name, column, target_table, target_column, on_delete) = match provider {
            Provider::Sqlite => (
                required(&row, 0)?,
                required(&row, 3)?,
                required(&row, 2)?,
                required(&row, 4)?,
                value(&row, 6),
            ),
            Provider::Postgres | Provider::Mysql => (
                required(&row, 0)?,
                required(&row, 1)?,
                required(&row, 2)?,
                required(&row, 3)?,
                None,
            ),
        };
        let entry = grouped.entry(name.clone()).or_insert_with(|| ForeignKey {
            name,
            columns: Vec::new(),
            target_table,
            target_columns: Vec::new(),
            on_delete,
        });
        entry.columns.push(column);
        entry.target_columns.push(target_column);
    }
    Ok(grouped.into_values().collect())
}

async fn fetch_rows(
    pool: &Pool,
    sql: &str,
    binds: Vec<Value>,
    width: usize,
) -> Result<Vec<Vec<Option<String>>>, Error> {
    let batch = pool
        .fetch_all_raw(Cow::Owned(sql.to_owned()), binds)
        .await?;
    match batch {
        RowBatch::Any(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        RowBatch::Postgres(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        RowBatch::Sqlite(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        RowBatch::Mysql(rows) => rows.iter().map(|row| row_cells(row, width)).collect(),
        #[cfg(feature = "sqlite-rusqlite")]
        RowBatch::Rusqlite(rows) => rows.iter().map(|row| rusqlite_cells(row, width)).collect(),
        _ => Err(Error::Message(
            "unsupported row batch for introspection".into(),
        )),
    }
}

fn row_cells<R>(row: &R, width: usize) -> Result<Vec<Option<String>>, Error>
where
    R: Row,
    usize: ColumnIndex<R>,
    String: for<'r> Decode<'r, R::Database> + Type<R::Database>,
    i64: for<'r> Decode<'r, R::Database> + Type<R::Database>,
{
    (0..width)
        .map(|idx| match row.try_get::<Option<String>, _>(idx) {
            Ok(value) => Ok(value),
            Err(text_error) => match row.try_get::<Option<i64>, _>(idx) {
                Ok(value) => Ok(value.map(|value| value.to_string())),
                Err(_) => Err(Error::Sqlx(text_error)),
            },
        })
        .collect()
}

#[cfg(feature = "sqlite-rusqlite")]
fn rusqlite_cells(
    row: &ruprizzle::rusqlite::Row,
    width: usize,
) -> Result<Vec<Option<String>>, Error> {
    use ruprizzle::rusqlite::types::Value as SqliteValue;

    Ok(row
        .0
        .iter()
        .take(width)
        .map(|value| match value {
            SqliteValue::Null => None,
            SqliteValue::Integer(value) => Some(value.to_string()),
            SqliteValue::Real(value) => Some(value.to_string()),
            SqliteValue::Text(value) => Some(value.clone()),
            SqliteValue::Blob(value) => Some(String::from_utf8_lossy(value).into_owned()),
        })
        .chain(std::iter::repeat(None))
        .take(width)
        .collect())
}

fn value(row: &[Option<String>], index: usize) -> Option<String> {
    row.get(index).cloned().flatten()
}

fn required(row: &[Option<String>], index: usize) -> Result<String, Error> {
    value(row, index)
        .ok_or_else(|| Error::Message(format!("database metadata column {index} is NULL")))
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}
