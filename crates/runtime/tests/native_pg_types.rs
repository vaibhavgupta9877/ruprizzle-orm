//! Regression guard for Phase B-5: every scalar type ruprizzle's Postgres
//! dialect emits in DDL must round-trip through the **native** Postgres pool.
//!
//! `sqlx::Any` cannot read six of these types (see the `pg_any_types` example,
//! round 1's F2). The native pool can. This test pins that, so a future change
//! that routes Postgres reads back through `Any` fails here.
//!
//! Skipped when `RUPRIZZLE_TEST_PG_URL` is unset, and panics instead when
//! `RUPRIZZLE_REQUIRE_DB` is set so a missing database cannot pass silently.

use ruprizzle::sqlx::Row;
use ruprizzle::types::chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use ruprizzle::types::{Decimal, Uuid};
use ruprizzle::{Executor, Pool, connect};
use ruprizzle_testkit::IsolatedSchema;
use serde_json::Value as JsonValue;

const DDL: &str = "CREATE TABLE typed (
    t TEXT NOT NULL,
    i INTEGER NOT NULL,
    b BIGINT NOT NULL,
    f DOUBLE PRECISION NOT NULL,
    bo BOOLEAN NOT NULL,
    by BYTEA NOT NULL,
    d NUMERIC NOT NULL,
    ts TIMESTAMPTZ NOT NULL,
    dt DATE NOT NULL,
    tm TIME NOT NULL,
    u UUID NOT NULL,
    j JSONB NOT NULL
)";

const INSERT: &str = "INSERT INTO typed VALUES (
    'x', 1, 2, 1.5, true, '\\x00ff', 1.50,
    '2024-01-02T03:04:05Z', '2024-01-02', '03:04:05',
    '00000000-0000-0000-0000-000000000001', '{\"k\": \"v\"}'
)";

fn require_db() -> bool {
    std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok_and(|v| v != "0" && !v.is_empty())
}

#[tokio::test]
async fn every_ddl_type_round_trips_on_native_postgres() {
    let base = match std::env::var("RUPRIZZLE_TEST_PG_URL")
        .ok()
        .filter(|u| !u.is_empty())
    {
        Some(base) => base,
        None if require_db() => {
            panic!("RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not")
        }
        None => {
            eprintln!("skipped: RUPRIZZLE_TEST_PG_URL is not set");
            return;
        }
    };

    let schema = IsolatedSchema::create(&base)
        .await
        .expect("isolated schema");
    let pool = connect(schema.url()).await.expect("connect");
    assert_eq!(pool.dialect().name(), "postgres");
    let Pool::Postgres(native) = &pool else {
        panic!("expected the native Postgres pool, got a different backend");
    };

    ruprizzle::sqlx::query(DDL)
        .execute(native)
        .await
        .expect("create table");
    ruprizzle::sqlx::query(INSERT)
        .execute(native)
        .await
        .expect("insert every type");

    let row = ruprizzle::sqlx::query("SELECT * FROM typed")
        .fetch_one(native)
        .await
        .expect("read every type through the native pool");

    assert_eq!(row.get::<String, _>("t"), "x");
    assert_eq!(row.get::<i32, _>("i"), 1);
    assert_eq!(row.get::<i64, _>("b"), 2);
    assert_eq!(row.get::<f64, _>("f"), 1.5);
    assert!(row.get::<bool, _>("bo"));
    assert_eq!(row.get::<Vec<u8>, _>("by"), vec![0x00, 0xff]);
    assert_eq!(row.get::<Decimal, _>("d"), Decimal::new(150, 2));
    assert_eq!(
        row.get::<DateTime<Utc>, _>("ts").to_rfc3339(),
        "2024-01-02T03:04:05+00:00"
    );
    assert_eq!(
        row.get::<NaiveDate, _>("dt"),
        NaiveDate::from_ymd_opt(2024, 1, 2).unwrap()
    );
    assert_eq!(
        row.get::<NaiveTime, _>("tm"),
        NaiveTime::from_hms_opt(3, 4, 5).unwrap()
    );
    assert_eq!(
        row.get::<Uuid, _>("u"),
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    );
    assert_eq!(row.get::<JsonValue, _>("j"), serde_json::json!({"k": "v"}));

    pool.close().await;
    schema.drop_now().await.expect("drop isolated schema");
}
