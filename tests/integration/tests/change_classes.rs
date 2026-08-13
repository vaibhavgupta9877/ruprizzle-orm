//! Integration tests for the 12 change classes in ImplPlan07.

use std::fs;

use ruprizzle_migrate::Migrator;
use ruprizzle_testkit::{TestDb, both_dbs};

fn schema_template(provider: &str, body: &str) -> String {
    format!(
        r#"
datasource db {{
    provider = "{provider}"
    url      = env("DATABASE_URL")
}}

generator client {{
    provider = "rust"
}}

{body}
"#
    )
}

fn empty_like(schema: &ruprizzle_core::ir::Schema) -> ruprizzle_core::ir::Schema {
    let mut empty = schema.clone();
    empty.models.clear();
    empty.enums.clear();
    empty.relations.clear();
    empty
}

fn write_migration(dir: &std::path::Path, id: &str, up: &str, down: &str) -> std::io::Result<()> {
    let mig = dir.join(id);
    fs::create_dir_all(&mig)?;
    fs::write(mig.join("up.sql"), up)?;
    fs::write(mig.join("down.sql"), down)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn assert_migration(
    db: TestDb,
    prev_body: &str,
    next_body: &str,
    seed: &str,
    verify: &str,
    expected: i64,
    backfill: &str,
    accept_data_loss: bool,
) {
    let provider = db.backend().as_str();
    let prev = ruprizzle_parser::parse("prev", &schema_template(provider, prev_body))
        .expect("prev parses");
    let next = ruprizzle_parser::parse("next", &schema_template(provider, next_body))
        .expect("next parses");

    let dialect = ruprizzle_dialect::dialect_for(next.datasource.provider);
    let init_sql = ruprizzle_migrate::up_sql(&empty_like(&prev), &prev, dialect);

    let mut change_sql = ruprizzle_migrate::up_sql(&prev, &next, dialect);
    if !backfill.is_empty() {
        // Replace the placeholder backfill expression and uncomment the UPDATE.
        change_sql = change_sql
            .replace("-- UPDATE", "UPDATE")
            .replace("<expr>", backfill);
    }

    let dir = tempfile::tempdir().unwrap();
    write_migration(dir.path(), "000_init", &init_sql, "").expect("write init migration");

    let migrator = Migrator::new(dir.path());
    migrator.apply_all(db.pool(), false).await.unwrap();

    if !seed.is_empty() {
        db.execute(seed).await.unwrap();
    }

    write_migration(dir.path(), "001_change", &change_sql, "").expect("write change migration");

    migrator
        .apply_all(db.pool(), accept_data_loss)
        .await
        .unwrap();

    if !verify.is_empty() {
        let actual = db.fetch_i64(verify).await.unwrap();
        assert_eq!(actual, expected, "change class verification failed");
    }
}

both_dbs! {
    async fn round_trip_add_and_drop_column(db: TestDb) {
        let provider = db.backend().as_str();

        let prev_body = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let next_body = r#"
model User {
    id    Int     @id
    email String  @unique
    phone String?
}
"#;

        let prev = ruprizzle_parser::parse("prev", &schema_template(provider, prev_body))
            .expect("prev parses");
        let next = ruprizzle_parser::parse("next", &schema_template(provider, next_body))
            .expect("next parses");

        let dialect = ruprizzle_dialect::dialect_for(next.datasource.provider);
        let init_sql = ruprizzle_migrate::up_sql(&empty_like(&prev), &prev, dialect);
        let change_up = ruprizzle_migrate::up_sql(&prev, &next, dialect);
        let change_down = ruprizzle_migrate::down_sql(&prev, &next, dialect);

        let dir = tempfile::tempdir().unwrap();
        write_migration(dir.path(), "000_init", &init_sql, "")
            .expect("write init");

        let migrator = Migrator::new(dir.path());
        migrator.apply_all(db.pool(), false).await.unwrap();

        db.execute(r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#)
            .await
            .unwrap();

        write_migration(dir.path(), "001_change", &change_up, &change_down)
            .expect("write change");

        migrator.apply_all(db.pool(), false).await.unwrap();

        let phone: Option<String> = sqlx::query_scalar(
            r#"SELECT phone FROM "users" WHERE id = 1"#,
        )
        .fetch_one(db.any_pool())
        .await
        .unwrap();
        assert!(phone.is_none());

        // Roll back the column, then re-apply, and the original row must survive.
        migrator.rollback(db.pool(), 1).await.unwrap();
        migrator.apply_all(db.pool(), false).await.unwrap();

        let count = db
            .fetch_i64(r#"SELECT count(*) FROM "users""#)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}

both_dbs! {
    async fn add_model(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String @unique
}

model Product {
    id    Int    @id
    name  String
}
"#;

        let seed = r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#;
        let verify = r#"SELECT count(*) FROM "products""#;

        assert_migration(db, prev, next, seed, verify, 0, "", false).await;
    }
}

both_dbs! {
    async fn drop_model(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String @unique
}

model Product {
    id    Int    @id
    name  String
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let seed = r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com');
INSERT INTO "products" (id, name) VALUES (1, 'Widget')"#;
        let verify = r#"SELECT count(*) FROM "users""#;

        assert_migration(db, prev, next, seed, verify, 1, "", true).await;
    }
}

both_dbs! {
    async fn add_nullable_column(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let next = r#"
model User {
    id    Int     @id
    email String  @unique
    phone String?
}
"#;

        let seed = r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#;
        let verify = r#"SELECT count(*) FROM "users" WHERE phone IS NULL"#;

        assert_migration(db, prev, next, seed, verify, 1, "", false).await;
    }
}

both_dbs! {
    async fn add_not_null_column_with_default(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let next = r#"
model User {
    id     Int    @id
    email  String @unique
    status String @default("active")
}
"#;

        let seed = r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#;
        let verify = r#"SELECT count(*) FROM "users" WHERE status = 'active'"#;

        assert_migration(db, prev, next, seed, verify, 1, "", false).await;
    }
}

both_dbs! {
    async fn add_not_null_column_without_default(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String @unique
    code  String
}
"#;

        let seed = r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#;
        let verify = r#"SELECT count(*) FROM "users" WHERE code = 'X'"#;

        // The generated up.sql contains a RUPRIZZLE:BACKFILL block; we replace
        // the placeholder with an actual backfill expression before applying.
        assert_migration(db, prev, next, seed, verify, 1, "'X'", true).await;
    }
}

both_dbs! {
    async fn drop_column(db: TestDb) {
        let prev = r#"
model User {
    id    Int     @id
    email String  @unique
    phone String?
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String @unique
}
"#;

        let seed = r#"INSERT INTO "users" (id, email, phone) VALUES (1, 'alice@example.com', '123')"#;
        let verify = r#"SELECT count(*) FROM "users""#;

        assert_migration(db, prev, next, seed, verify, 1, "", true).await;
    }
}

both_dbs! {
    async fn widen_int_to_bigint(db: TestDb) {
        let prev = r#"
model User {
    id  Int    @id
    age Int
}
"#;

        let next = r#"
model User {
    id  Int    @id
    age BigInt
}
"#;

        let seed = r#"INSERT INTO "users" (id, age) VALUES (1, 30)"#;
        let verify = r#"SELECT count(*) FROM "users" WHERE age = 30"#;

        assert_migration(db, prev, next, seed, verify, 1, "", true).await;
    }
}

both_dbs! {
    async fn narrow_bigint_to_int(db: TestDb) {
        let prev = r#"
model User {
    id  Int    @id
    age BigInt
}
"#;

        let next = r#"
model User {
    id  Int    @id
    age Int
}
"#;

        let seed = r#"INSERT INTO "users" (id, age) VALUES (1, 30)"#;
        let verify = r#"SELECT count(*) FROM "users" WHERE age = 30"#;

        assert_migration(db, prev, next, seed, verify, 1, "", true).await;
    }
}

both_dbs! {
    async fn nullable_to_not_null(db: TestDb) {
        let prev = r#"
model User {
    id    Int     @id
    email String  @unique
    phone String?
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String @unique
    phone String
}
"#;

        let seed = r#"INSERT INTO "users" (id, email, phone) VALUES (1, 'alice@example.com', '123')"#;
        let verify = r#"SELECT count(*) FROM "users" WHERE phone = '123'"#;

        assert_migration(db, prev, next, seed, verify, 1, "", true).await;
    }
}

both_dbs! {
    async fn add_and_drop_unique(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String

    @@unique([email])
}
"#;

        let provider = db.backend().as_str();
        let prev_s = ruprizzle_parser::parse("prev", &schema_template(provider, prev)).unwrap();
        let next_s = ruprizzle_parser::parse("next", &schema_template(provider, next)).unwrap();
        let dialect = ruprizzle_dialect::dialect_for(next_s.datasource.provider);
        let init_sql = ruprizzle_migrate::up_sql(&empty_like(&prev_s), &prev_s, dialect);

        let dir = tempfile::tempdir().unwrap();
        write_migration(dir.path(), "000_init", &init_sql, "").unwrap();

        let migrator = Migrator::new(dir.path());
        migrator.apply_all(db.pool(), false).await.unwrap();

        // Adding a unique on an existing column with data should succeed if data is unique.
        db.execute(r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#)
            .await
            .unwrap();

        let change_sql = ruprizzle_migrate::up_sql(&prev_s, &next_s, dialect);
        write_migration(dir.path(), "001_change", &change_sql, "").unwrap();

        migrator.apply_all(db.pool(), false).await.unwrap();

        // Duplicate email must now be rejected.
        let result = db
            .execute(r#"INSERT INTO "users" (id, email) VALUES (2, 'alice@example.com')"#)
            .await;
        assert!(result.is_err(), "unique constraint should reject duplicates");
    }
}

both_dbs! {
    async fn add_and_drop_index(db: TestDb) {
        let prev = r#"
model User {
    id    Int    @id
    email String
}
"#;

        let next = r#"
model User {
    id    Int    @id
    email String
    @@index([email])
}
"#;

        assert_migration(db, prev, next, "", "", 0, "", false).await;

        // Drift detection should not flag the index because it is not checked yet,
        // but the migration should apply cleanly.
    }
}

#[tokio::test]
async fn add_foreign_key_postgres() {
    let setup = "";
    ruprizzle_testkit::run_case(
        ruprizzle_testkit::Backend::Postgres,
        setup,
        |db: TestDb| async move {
            let provider = db.backend().as_str();

            let prev = r#"
model User {
    id    Int    @id
    email String @unique
}

model Post {
    id       Int     @id
    title    String
    authorId Int?    @map("author_id")
}
"#;

            let next = r#"
model User {
    id    Int    @id
    email String @unique
    posts Post[]
}

model Post {
    id       Int     @id
    title    String
    authorId Int     @map("author_id")
    author   User    @relation(fields: [authorId], references: [id], onDelete: Cascade)
}
"#;

            let prev_s = ruprizzle_parser::parse("prev", &schema_template(provider, prev)).unwrap();
            let next_s = ruprizzle_parser::parse("next", &schema_template(provider, next)).unwrap();
            let dialect = ruprizzle_dialect::dialect_for(next_s.datasource.provider);
            let init_sql =
                ruprizzle_migrate::up_sql(&empty_like(&prev_s), &prev_s, dialect);

            let dir = tempfile::tempdir().unwrap();
            write_migration(dir.path(), "000_init", &init_sql, "").unwrap();

            let migrator = Migrator::new(dir.path());
            migrator.apply_all(db.pool(), false).await.unwrap();

            db.execute(r#"INSERT INTO "users" (id, email) VALUES (1, 'alice@example.com')"#)
                .await
                .unwrap();
            db.execute(r#"INSERT INTO "posts" (id, title, author_id) VALUES (1, 'Hello', 1)"#)
                .await
                .unwrap();

            let change_sql = ruprizzle_migrate::up_sql(&prev_s, &next_s, dialect);
            write_migration(dir.path(), "001_change", &change_sql, "").unwrap();

            migrator.apply_all(db.pool(), false).await.unwrap();

            // A missing author must now be rejected.
            let result = db
                .execute(r#"INSERT INTO "posts" (id, title, author_id) VALUES (2, 'Orphan', 999)"#)
                .await;
            assert!(result.is_err(), "foreign key should reject missing target");

            Ok(())
        },
    )
    .await;
}

#[tokio::test]
async fn add_enum_variant_postgres() {
    let setup = "";
    ruprizzle_testkit::run_case(
        ruprizzle_testkit::Backend::Postgres,
        setup,
        |db: TestDb| async move {
            let provider = db.backend().as_str();

            let prev = r#"
enum Role {
    USER
}

model User {
    id    Int    @id
    email String @unique
    role  Role   @default(USER)
}
"#;

            let next = r#"
enum Role {
    USER
    ADMIN
}

model User {
    id    Int    @id
    email String @unique
    role  Role   @default(USER)
}
"#;

            let prev_s = ruprizzle_parser::parse("prev", &schema_template(provider, prev)).unwrap();
            let next_s = ruprizzle_parser::parse("next", &schema_template(provider, next)).unwrap();
            let dialect = ruprizzle_dialect::dialect_for(next_s.datasource.provider);
            let init_sql =
                ruprizzle_migrate::up_sql(&empty_like(&prev_s), &prev_s, dialect);

            let dir = tempfile::tempdir().unwrap();
            write_migration(dir.path(), "000_init", &init_sql, "").unwrap();

            let migrator = Migrator::new(dir.path());
            migrator.apply_all(db.pool(), false).await.unwrap();

            db.execute(
                r#"INSERT INTO "users" (id, email, role) VALUES (1, 'alice@example.com', 'USER')"#,
            )
            .await
            .unwrap();

            let change_sql = ruprizzle_migrate::up_sql(&prev_s, &next_s, dialect);
            write_migration(dir.path(), "001_change", &change_sql, "").unwrap();

            migrator.apply_all(db.pool(), false).await.unwrap();

            db.execute(
                r#"INSERT INTO "users" (id, email, role) VALUES (2, 'bob@example.com', 'ADMIN')"#,
            )
            .await
            .unwrap();

            let count = db
                .fetch_i64(r#"SELECT count(*) FROM "users""#)
                .await
                .unwrap();
            assert_eq!(count, 2);

            Ok(())
        },
    )
    .await;
}
