//! Path-based JSON filtering and ordering across dialects.
//!
//! This test exercises multi-segment JSON paths and ordering through the
//! `sqlx::Any` driver on SQLite and MySQL, and verifies generated SQL on
//! Postgres. Postgres runtime execution is intentionally limited to SQL
//! snapshots because the `Any` driver binds `Value::Json` as text, which does
//! not round-trip into `JSONB` columns without the P0-1 rich-type work.

use std::borrow::Cow;

use ruprizzle::sqlx::Row;
use ruprizzle::{Column, Executor, InsertQuery, Model, SelectQuery};
use ruprizzle_testkit::{Backend, both_dbs};

#[derive(Debug, Clone, PartialEq, Default)]
struct Item {
    id: i64,
    meta: serde_json::Value,
}

impl Model for Item {
    const TABLE: &'static str = "items";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "meta"];
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::any::AnyRow> for Item {
    fn from_row(row: &'r ruprizzle::sqlx::any::AnyRow) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ruprizzle::decode::direct(row, "id")?,
            meta: {
                let s: String = row.try_get("meta")?;
                serde_json::from_str(&s).map_err(|e| ruprizzle::sqlx::Error::Decode(Box::new(e)))?
            },
        })
    }
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::postgres::PgRow> for Item {
    fn from_row(row: &'r ruprizzle::sqlx::postgres::PgRow) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ruprizzle::decode::direct(row, "id")?,
            meta: ruprizzle::decode::json(row, "meta")?,
        })
    }
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::sqlite::SqliteRow> for Item {
    fn from_row(
        row: &'r ruprizzle::sqlx::sqlite::SqliteRow,
    ) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ruprizzle::decode::direct(row, "id")?,
            meta: ruprizzle::decode::json(row, "meta")?,
        })
    }
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::mysql::MySqlRow> for Item {
    fn from_row(row: &'r ruprizzle::sqlx::mysql::MySqlRow) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ruprizzle::decode::direct(row, "id")?,
            meta: ruprizzle::decode::json(row, "meta")?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Item {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            meta: ::ruprizzle::rusqlite::get::<serde_json::Value>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Item {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            meta: row.get::<serde_json::Value>(1)?,
        })
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Item {
    fn from_tokio_postgres_row(
        row: &ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row
                .try_get::<usize, i64>(0)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            meta: row
                .try_get::<usize, serde_json::Value>(1)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

const ID: Column<Item, i64> = Column::new("items", "id");
const META: Column<Item, serde_json::Value> = Column::new("items", "meta");

both_dbs! {
    setup = "";
    async fn json_path_filter_and_order(db: TestDb) {
        let create_sql = match db.backend() {
            Backend::Postgres => "CREATE TABLE items (id BIGINT PRIMARY KEY, meta JSONB)",
            Backend::MySql => "CREATE TABLE items (id BIGINT PRIMARY KEY, meta JSON)",
            Backend::Sqlite => "CREATE TABLE items (id INTEGER PRIMARY KEY, meta TEXT)",
        };
        Executor::execute_raw(db.pool(), Cow::Borrowed(create_sql), Vec::new()).await?;

        let row1 = r#"{"status":"active","nested":{"score":5,"name":"x"},"priority":3,"tags":["a","b"],"flag":true}"#;
        let row2 = r#"{"status":"inactive","nested":{"score":15,"name":"y"},"priority":1,"tags":["c"],"flag":true}"#;
        let row3 = r#"{"status":"active","nested":{"score":20,"name":"z"},"priority":5,"tags":[],"flag":false}"#;
        let row10 = r#"[{"name":"first"}]"#;

        for (id, payload) in [(1, row1), (2, row2), (3, row3), (10, row10)] {
            let sql = format!("INSERT INTO items (id, meta) VALUES ({id}, '{payload}')");
            Executor::execute_raw(db.pool(), Cow::Owned(sql), Vec::new()).await?;
        }

        // Every query should compile on every backend, even if we cannot
        // execute them all through `sqlx::Any`.
        let by_status = SelectQuery::<Item>::new(db.pool())
            .filter(META.get("status").eq("active"));
        let by_nested_score = SelectQuery::<Item>::new(db.pool())
            .filter(META.get("nested").get("score").gt(10));
        let by_array_name = SelectQuery::<Item>::new(db.pool())
            .filter(META.at(0).get_text("name").eq("first"));
        let ordered_by_priority = SelectQuery::<Item>::new(db.pool())
            .filter(META.has_key("priority"))
            .order_by(META.get("priority").desc());
        let has_tags = SelectQuery::<Item>::new(db.pool())
            .filter(META.has_key("tags"));
        let contains_flag = SelectQuery::<Item>::new(db.pool())
            .filter(META.contains(serde_json::json!({"flag": true})));

        if db.backend() == Backend::Postgres {
            // Verify SQL generation; runtime execution is gated on P0-1.
            assert!(by_status.to_sql()?.sql.contains("->'status'"));
            assert!(by_nested_score.to_sql()?.sql.contains("#>'{\"nested\",\"score\"}'"));
            assert!(by_array_name.to_sql()?.sql.contains("#>>'{0,\"name\"}'"));
            assert!(ordered_by_priority.to_sql()?.sql.contains("->'priority'"));
            assert!(has_tags.to_sql()?.sql.contains("? $1"));
            assert!(contains_flag.to_sql()?.sql.contains("@> $1::jsonb"));
            return Ok(());
        }

        // SQLite's json_extract() returns de-quoted scalar text, so string
        // equality works with get_text; numeric comparisons still work with get.
        let active_status = if db.backend() == Backend::Sqlite {
            SelectQuery::<Item>::new(db.pool())
                .filter(META.get_text("status").eq("active"))
                .fetch_all()
                .await?
        } else {
            by_status.fetch_all().await?
        };
        let active_ids: Vec<_> = active_status.iter().map(|i| i.id).collect();
        assert!(active_ids.contains(&1));
        assert!(active_ids.contains(&3));
        assert!(!active_ids.contains(&2));

        let high_score = by_nested_score.fetch_all().await?;
        let high_ids: Vec<_> = high_score.iter().map(|i| i.id).collect();
        assert!(high_ids.contains(&2));
        assert!(high_ids.contains(&3));
        assert!(!high_ids.contains(&1));

        let array_match = by_array_name.fetch_all().await?;
        assert_eq!(array_match.len(), 1);
        assert_eq!(array_match[0].id, 10);

        let ordered = ordered_by_priority.fetch_all().await?;
        let ordered_ids: Vec<_> = ordered.iter().map(|i| i.id).collect();
        assert_eq!(ordered_ids[0], 3);
        assert_eq!(ordered_ids[1], 1);
        assert_eq!(ordered_ids[2], 2);

        let tagged = has_tags.fetch_all().await?;
        let tagged_ids: Vec<_> = tagged.iter().map(|i| i.id).collect();
        assert!(tagged_ids.contains(&1));
        assert!(tagged_ids.contains(&2));
        assert!(tagged_ids.contains(&3));

        let flagged = contains_flag.fetch_all().await?;
        let flagged_ids: Vec<_> = flagged.iter().map(|i| i.id).collect();
        if db.backend() == Backend::MySql {
            // MySQL and Postgres JSON containment checks the value.
            assert!(flagged_ids.contains(&1));
            assert!(flagged_ids.contains(&2));
            assert!(!flagged_ids.contains(&3));
        } else {
            // SQLite uses a partial approximation: top-level key existence.
            // All rows with a `flag` key match.
            assert!(flagged_ids.contains(&1));
            assert!(flagged_ids.contains(&2));
            assert!(flagged_ids.contains(&3));
        }

        // Demonstrate the `at` -> `get_text` SQL for ordering as well.
        let _ = SelectQuery::<Item>::new(db.pool())
            .filter(META.at(0).get("name").eq("first"))
            .to_sql()?;

        // Smoke-test that insert round-trips through the typed builder.
        let inserted: Item = InsertQuery::<Item>::new(db.pool())
            .set(ID, 100_i64)
            .set(META, serde_json::json!({"status": "new"}))
            .exec()
            .await?;
        assert_eq!(inserted.id, 100);
        assert_eq!(inserted.meta, serde_json::json!({"status": "new"}));
    }
}
