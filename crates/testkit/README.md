# ruprizzle-testkit

[![Crates.io](https://img.shields.io/crates/v/ruprizzle-testkit.svg)](https://crates.io/crates/ruprizzle-testkit)
[![docs.rs](https://docs.rs/ruprizzle-testkit/badge.svg)](https://docs.rs/ruprizzle-testkit)
[![License](https://img.shields.io/crates/l/ruprizzle-testkit.svg)](https://github.com/vaibhavgupta9877/ruprizzle-orm)

Dual-database test harness for `ruprizzle-orm`.

`ruprizzle-testkit` provides the `TestDb` abstraction and the `both_dbs!` macro used by the integration suite. It lets a single test run against Postgres, MySQL/MariaDB, and SQLite by spawning isolated databases, applying schemas, and cleaning up afterwards. It is an internal testing tool and is not published on crates.io (`publish = false` in its `Cargo.toml`).

## Responsibilities

- **Backend abstraction** — `TestDb` hides the difference between PostgreSQL, MySQL/MariaDB, and SQLite.
- **Schema isolation** — Postgres tests create a temporary schema; MySQL/MariaDB and SQLite tests use a temporary database or file.
- **`both_dbs!` macro** — run the same async test body on both backends.
- **Connection helpers** — connect to `DATABASE_URL`, `RUPRIZZLE_TEST_PG_URL`, or the default local Postgres.

## Example

```rust
use ruprizzle_testkit::{TestDb, both_dbs};

both_dbs! {
    async fn create_and_count(db: TestDb) {
        db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .await
            .unwrap();
        db.execute("INSERT INTO users (name) VALUES ('Alice')")
            .await
            .unwrap();
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(row.0, 1);
    }
}
```

- [Repository](https://github.com/vaibhavgupta9877/ruprizzle-orm)
- [Documentation](https://docs.rs/ruprizzle-testkit)
- [Project homepage](https://vaibhavgupta9877.github.io/ruprizzle-orm)
- [Changelog](https://github.com/vaibhavgupta9877/ruprizzle-orm/blob/main/CHANGELOG.md)

## Keywords

orm, sql, database, testing, harness
