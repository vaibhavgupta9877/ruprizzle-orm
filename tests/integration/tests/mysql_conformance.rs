//! MySQL-specific conformance tests.
//!
//! Each case is generated for all three backends by the test harness, but the
//! meaningful work is gated to `Backend::MySql` so PostgreSQL and SQLite test
//! runners still pass when MySQL is unavailable.

use ruprizzle_testkit::{Backend, all_dbs};

all_dbs! {
    async fn upsert_on_duplicate_key_update(db: TestDb) {
        if db.backend() != Backend::MySql {
            return Ok(());
        }

        db.execute(
            "DROP TABLE IF EXISTS mysql_upsert_products"
        ).await?;
        db.execute(
            "CREATE TABLE mysql_upsert_products (
                id INT PRIMARY KEY,
                sku VARCHAR(100) NOT NULL,
                qty INT NOT NULL DEFAULT 0
            )"
        ).await?;

        db.execute(
            "INSERT INTO mysql_upsert_products (id, sku, qty) VALUES (1, 'old', 0)"
        ).await?;

        db.execute(
            "INSERT INTO mysql_upsert_products (id, sku, qty) VALUES (1, 'new', 10)
             ON DUPLICATE KEY UPDATE sku = 'new', qty = 10"
        ).await?;

        let sku = db.fetch_string("SELECT sku FROM mysql_upsert_products WHERE id = 1").await?;
        let qty = db.fetch_i64("SELECT qty FROM mysql_upsert_products WHERE id = 1").await?;

        assert_eq!(sku, "new");
        assert_eq!(qty, 10);
    }
}

all_dbs! {
    async fn uuid_round_trip_as_char36(db: TestDb) {
        if db.backend() != Backend::MySql {
            return Ok(());
        }

        db.execute("DROP TABLE IF EXISTS mysql_uuid_users").await?;
        db.execute(
            "CREATE TABLE mysql_uuid_users (
                id CHAR(36) PRIMARY KEY,
                name VARCHAR(100) NOT NULL
            )"
        ).await?;

        let id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11";
        db.execute(&format!(
            "INSERT INTO mysql_uuid_users (id, name) VALUES ('{id}', 'Alice')"
        )).await?;

        let name = db.fetch_string(
            "SELECT name FROM mysql_uuid_users WHERE id = 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'"
        ).await?;
        assert_eq!(name, "Alice");
    }
}

all_dbs! {
    async fn decimal_native_storage(db: TestDb) {
        if db.backend() != Backend::MySql {
            return Ok(());
        }

        db.execute("DROP TABLE IF EXISTS mysql_decimal_prices").await?;
        db.execute(
            "CREATE TABLE mysql_decimal_prices (
                id INT PRIMARY KEY,
                amount DECIMAL(19,4) NOT NULL
            )"
        ).await?;

        db.execute(
            "INSERT INTO mysql_decimal_prices (id, amount) VALUES (1, 123.4567)"
        ).await?;

        let amount = db.fetch_string(
            "SELECT amount FROM mysql_decimal_prices WHERE id = 1"
        ).await?;
        assert_eq!(amount, "123.4567");
    }
}

all_dbs! {
    async fn json_array_contains_and_overlaps(db: TestDb) {
        if db.backend() != Backend::MySql {
            return Ok(());
        }

        db.execute("DROP TABLE IF EXISTS mysql_json_articles").await?;
        db.execute(
            "CREATE TABLE mysql_json_articles (
                id INT PRIMARY KEY,
                tags JSON NOT NULL
            )"
        ).await?;

        db.execute(
            "INSERT INTO mysql_json_articles (id, tags) VALUES
                (1, '[\"rust\", \"orm\"]'),
                (2, '[\"sql\"]')"
        ).await?;

        let contains = db.fetch_i64(
            "SELECT count(*) FROM mysql_json_articles WHERE JSON_CONTAINS(tags, '[\"rust\"]')"
        ).await?;
        assert_eq!(contains, 1);

        let overlaps = db.fetch_i64(
            "SELECT count(*) FROM mysql_json_articles WHERE JSON_OVERLAPS(tags, '[\"orm\", \"sql\"]')"
        ).await?;
        assert_eq!(overlaps, 2);
    }
}
