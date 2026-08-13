//! Rich-type round-trip: `Uuid`, `DateTime<Utc>`, `Decimal`, `Json`.
//!
//! On SQLite this should pass end-to-end. On Postgres through `sqlx::Any` it
//! should currently fail because the `Any` driver cannot read (or correctly
//! bind) those native types — this is the F2 reproduction the plan calls P0-1.
//! After the native-backend work (P2-2) the Postgres branch should be changed
//! from `assert!(insert.is_err())` to the same round-trip assertions as SQLite.

use ruprizzle::sqlx::Row;
use ruprizzle::types::chrono::{DateTime, Utc};
use ruprizzle::types::{Decimal, Uuid};
use ruprizzle::{Column, Executor, InsertQuery, Model, Pool, SelectQuery, connect, decode};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq)]
struct Event {
    id: Uuid,
    created_at: DateTime<Utc>,
    price: Decimal,
    meta: JsonValue,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            id: Uuid::default(),
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            price: Decimal::ZERO,
            meta: JsonValue::Null,
        }
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Event);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Event {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<Uuid>(row, 0)?,
            created_at: ::ruprizzle::rusqlite::get::<DateTime<Utc>>(row, 1)?,
            price: ::ruprizzle::rusqlite::get::<Decimal>(row, 2)?,
            meta: ::ruprizzle::rusqlite::get::<serde_json::Value>(row, 3)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Event {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<Uuid>(0)?,
            created_at: row.get::<DateTime<Utc>>(1)?,
            price: row.get::<Decimal>(2)?,
            meta: row.get::<serde_json::Value>(3)?,
        })
    }
}

impl Model for Event {
    const TABLE: &'static str = "events";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "created_at", "price", "meta"];
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::any::AnyRow> for Event {
    fn from_row(row: &'r ruprizzle::sqlx::any::AnyRow) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: decode::text(row, "id")?,
            created_at: decode::text(row, "created_at")?,
            price: decode::text(row, "price")?,
            meta: {
                let s: String = row.try_get("meta")?;
                serde_json::from_str(&s).map_err(|e| ruprizzle::sqlx::Error::Decode(Box::new(e)))?
            },
        })
    }
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::postgres::PgRow> for Event {
    fn from_row(row: &'r ruprizzle::sqlx::postgres::PgRow) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: decode::rich(row, "id")?,
            created_at: decode::rich(row, "created_at")?,
            price: decode::rich(row, "price")?,
            meta: decode::json(row, "meta")?,
        })
    }
}

impl<'r> ruprizzle::sqlx::FromRow<'r, ruprizzle::sqlx::sqlite::SqliteRow> for Event {
    fn from_row(
        row: &'r ruprizzle::sqlx::sqlite::SqliteRow,
    ) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: decode::rich(row, "id")?,
            created_at: decode::rich(row, "created_at")?,
            price: decode::text(row, "price")?,
            meta: decode::json(row, "meta")?,
        })
    }
}

const ID: Column<Event, Uuid> = Column::new("events", "id");
const CREATED_AT: Column<Event, DateTime<Utc>> = Column::new("events", "created_at");
const PRICE: Column<Event, Decimal> = Column::new("events", "price");
const META: Column<Event, JsonValue> = Column::new("events", "meta");

async fn fresh_pool() -> (Pool, bool) {
    let (url, is_pg) = if let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") {
        (url, true)
    } else if std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok() {
        panic!("RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not");
    } else {
        let dir = std::env::current_dir().unwrap().join("target/runtime-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("rich_types_{}.sqlite", std::process::id()));
        let file = path.to_str().unwrap().replace('\\', "/");
        let driver = if std::env::var("RUPRIZZLE_TEST_RUSQLITE").is_ok() {
            "&driver=rusqlite"
        } else {
            ""
        };
        let url = format!("sqlite:///{}?mode=rwc{}", file, driver);
        (url, false)
    };

    let pool = connect(&url).await.unwrap();
    let dialect = pool.dialect().name();

    if dialect == "postgres" {
        Executor::execute_raw(&pool, "DROP TABLE IF EXISTS events".into(), Vec::new())
            .await
            .unwrap();
        Executor::execute_raw(
            &pool,
            "CREATE TABLE events (
                id UUID PRIMARY KEY,
                created_at TIMESTAMPTZ NOT NULL,
                price NUMERIC NOT NULL,
                meta JSONB
            )"
            .into(),
            Vec::new(),
        )
        .await
        .unwrap();
    } else {
        Executor::execute_raw(&pool, "DROP TABLE IF EXISTS events".into(), Vec::new())
            .await
            .unwrap();
        Executor::execute_raw(
            &pool,
            "CREATE TABLE events (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                price TEXT NOT NULL,
                meta TEXT
            )"
            .into(),
            Vec::new(),
        )
        .await
        .unwrap();
    }

    (pool, is_pg)
}

#[tokio::test]
async fn rich_types_round_trip() {
    let (pool, _is_pg) = fresh_pool().await;

    let id = Uuid::nil();
    let created_at = Utc::now();
    let price = Decimal::new(150, 2); // 1.50
    let meta = serde_json::json!({"k": "v"});

    let inserted: Event = InsertQuery::<Event>::new(&pool)
        .set(ID, id)
        .set(CREATED_AT, created_at)
        .set(PRICE, price)
        .set(META, meta.clone())
        .exec()
        .await
        .unwrap();
    assert_eq!(inserted.id, id);
    assert_eq!(inserted.price, price);
    assert_eq!(inserted.meta, meta);
    assert_eq!(inserted.created_at.timestamp(), created_at.timestamp());

    let rows: Vec<Event> = SelectQuery::<Event>::new(&pool)
        .filter(ID.eq(id))
        .fetch_all()
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, id);
    assert_eq!(row.price, price);
    assert_eq!(row.meta, meta);
    assert_eq!(row.created_at.timestamp(), created_at.timestamp());
}
