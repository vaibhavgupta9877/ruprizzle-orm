//! Rich-type round-trip: `Uuid`, `DateTime<Utc>`, `Decimal`, `Json`.
//!
//! On SQLite this should pass end-to-end. On Postgres through `sqlx::Any` it
//! should currently fail because the `Any` driver cannot read (or correctly
//! bind) those native types — this is the F2 reproduction the plan calls P0-1.
//! After the native-backend work (P2-2) the Postgres branch should be changed
//! from `assert!(insert.is_err())` to the same round-trip assertions as SQLite.

use ruprizzle::{Column, Executor, InsertQuery, Model, Pool, SelectQuery, connect, decode};
use ruprizzle::types::chrono::{DateTime, Utc};
use ruprizzle::types::{Decimal, Uuid};
use ruprizzle::sqlx::any::AnyRow;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq)]
struct Event {
    id: Uuid,
    created_at: DateTime<Utc>,
    price: Decimal,
    meta: JsonValue,
}

impl Model for Event {
    const TABLE: &'static str = "events";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "created_at", "price", "meta"];
}

impl<'r> ruprizzle::sqlx::FromRow<'r, AnyRow> for Event {
    fn from_row(row: &'r AnyRow) -> Result<Self, ruprizzle::sqlx::Error> {
        Ok(Self {
            id: decode::text(row, "id")?,
            created_at: decode::text(row, "created_at")?,
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
        let url = format!("sqlite:///{}?mode=rwc", file);
        (url, false)
    };

    let pool = connect(&url).await.unwrap();
    let dialect = pool.dialect().name();

    if dialect == "postgres" {
        sqlx::query("DROP TABLE IF EXISTS events").execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE events (
                id UUID PRIMARY KEY,
                created_at TIMESTAMPTZ NOT NULL,
                price NUMERIC NOT NULL,
                meta JSONB
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
    } else {
        sqlx::query("DROP TABLE IF EXISTS events").execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE events (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                price TEXT NOT NULL,
                meta TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    (pool, is_pg)
}

#[tokio::test]
async fn rich_types_round_trip() {
    let (pool, is_pg) = fresh_pool().await;

    let id = Uuid::nil();
    let created_at = Utc::now();
    let price = Decimal::new(150, 2); // 1.50
    let meta = serde_json::json!({"k": "v"});

    let insert = InsertQuery::<Event>::new(&pool)
        .set(ID, id)
        .set(CREATED_AT, created_at)
        .set(PRICE, price)
        .set(META, meta.clone())
        .exec()
        .await;

    if is_pg {
        assert!(
            insert.is_err(),
            "Postgres rich-type insert through sqlx::Any is expected to fail until P2-2: {insert:?}"
        );
        let err = insert.unwrap_err().to_string();
        assert!(
            err.contains("Any driver") || err.contains("uuid") || err.contains("jsonb"),
            "expected an Any-driver or Postgres type error, got: {err}"
        );
        return;
    }

    let inserted: Event = insert.unwrap();
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
