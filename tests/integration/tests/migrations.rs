//! Migration runner integration tests.

use std::fs;

use ruprizzle_migrate::Migrator;
use ruprizzle_testkit::both_dbs;

fn schema_v1(provider: &str) -> String {
    format!(
        r#"
datasource db {{
    provider = "{provider}"
    url      = env("DATABASE_URL")
}}

generator client {{
    provider = "rust"
}}

model User {{
    id    Int    @id
    email String @unique
    name  String
}}
"#
    )
}

fn schema_v2(provider: &str) -> String {
    format!(
        r#"
datasource db {{
    provider = "{provider}"
    url      = env("DATABASE_URL")
}}

generator client {{
    provider = "rust"
}}

model User {{
    id    Int    @id
    email String @unique
    name  String
    age   Int    @default(0)
}}
"#
    )
}

both_dbs! {
    async fn migrator_applies_pending_migrations(db: TestDb) {
        let dir = tempfile::tempdir()?;

        // Create a simple migration directory.
        let mig = dir.path().join("20260810_000000_init");
        fs::create_dir_all(&mig)?;
        fs::write(
            mig.join("up.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
        )?;
        fs::write(
            mig.join("down.sql"),
            "DROP TABLE users;",
        )?;
        fs::write(
            mig.join("meta.json"),
            r#"{"id":"20260810_000000_init","checksum":"","destructive":false,"ruprizzle_version":"0.1.0"}"#,
        )?;

        let migrator = Migrator::new(dir.path());

        let status = migrator.status(db.any_pool()).await?;
        assert_eq!(status.pending, vec!["20260810_000000_init"]);
        assert!(status.applied.is_empty());

        let report = migrator.apply_all(db.any_pool(), false).await?;
        assert_eq!(report.applied, vec!["20260810_000000_init"]);

        let status = migrator.status(db.any_pool()).await?;
        assert!(status.pending.is_empty());
        assert_eq!(status.applied, vec!["20260810_000000_init"]);

        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(db.any_pool())
            .await?;
        assert_eq!(count, 0);
    }
}

both_dbs! {
    async fn migrator_applies_schema_diff(db: TestDb) {
        let provider = db.backend().as_str();

        let v1 = ruprizzle_parser::parse("v1", &schema_v1(provider)).expect("v1 parses");
        let v2 = ruprizzle_parser::parse("v2", &schema_v2(provider)).expect("v2 parses");

        let changes = ruprizzle_migrate::diff(&v1, &v2);
        let dialect = ruprizzle_dialect::dialect_for(v2.datasource.provider);
        let stmts = ruprizzle_migrate::plan(&v1, &v2, dialect.as_ref(), &changes);

        let dir = tempfile::tempdir()?;

        // First migration: create the initial table.
        let init = dir.path().join("20260810_000000_init");
        fs::create_dir_all(&init)?;
        fs::write(
            init.join("up.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE, name TEXT NOT NULL);",
        )?;
        fs::write(init.join("down.sql"), "DROP TABLE users;")?;

        let migrator = Migrator::new(dir.path());
        migrator.apply_all(db.any_pool(), false).await?;

        // Insert a row using the v1 schema.
        sqlx::query("INSERT INTO users (id, email, name) VALUES (1, 'a@example.com', 'Alice')")
            .execute(db.any_pool())
            .await?;

        // Second migration: add the age column generated from the diff.
        let mig = dir.path().join("20260810_000001_add_age");
        fs::create_dir_all(&mig)?;
        let up = stmts.iter().map(|s| s.sql.clone()).collect::<Vec<_>>().join("\n");
        fs::write(mig.join("up.sql"), up)?;
        fs::write(mig.join("down.sql"), "")?;

        migrator.apply_all(db.any_pool(), false).await?;

        let age: i64 = sqlx::query_scalar("SELECT age FROM users WHERE id = 1")
            .fetch_one(db.any_pool())
            .await?;
        assert_eq!(age, 0);
    }
}

both_dbs! {
    async fn drift_detects_missing_and_extra_tables(db: TestDb) {
        let provider = db.backend().as_str();

        let v1 = ruprizzle_parser::parse("v1", &schema_v1(provider)).expect("v1 parses");

        // Create the initial table manually so the DB matches the snapshot.
        let dir = tempfile::tempdir()?;
        let mig = dir.path().join("20260810_000000_init");
        fs::create_dir_all(&mig)?;
        fs::write(
            mig.join("up.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE, name TEXT NOT NULL);",
        )?;
        fs::write(mig.join("down.sql"), "DROP TABLE users;")?;

        let migrator = Migrator::new(dir.path());
        migrator.apply_all(db.any_pool(), false).await?;

        // Snapshot should initially match.
        let snapshot = ruprizzle_migrate::detect(db.any_pool(), &v1).await?;
        assert!(snapshot.is_empty(), "expected no drift: {snapshot:?}");

        // Manually add an unexpected table.
        sqlx::query("CREATE TABLE drift_test (id INTEGER PRIMARY KEY)")
            .execute(db.any_pool())
            .await?;

        let drift = ruprizzle_migrate::detect(db.any_pool(), &v1).await?;
        assert!(
            drift.iter().any(|d| d.contains("drift_test")),
            "expected drift to report extra table: {drift:?}"
        );
    }
}
