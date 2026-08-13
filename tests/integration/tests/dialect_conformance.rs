//! Conformance suite: run generated DDL and DML against real backends.
//!
//! These tests use the `ruprizzle_testkit::both_dbs!` macro so every case is
//! executed against both PostgreSQL and SQLite.

use ruprizzle_core::ir::Schema;
use ruprizzle_dialect::{DbDialect, dialect_for, full_alter_column, full_create_table};
use ruprizzle_parser::parse;
use ruprizzle_testkit::{TestDb, both_dbs};

fn conformance_schema() -> Schema {
    let source = r#"
        datasource db {
            provider = "postgres"
            url      = env("DATABASE_URL")
        }

        generator client {
            provider = "prisma-client-js"
        }

        model User {
            id      String  @id @default(uuid4())
            email   String  @unique
            name    String?
            isAdmin Boolean @default(false) @map("is_admin")
            posts   Post[]
        }

        model Post {
            id        String  @id @default(uuid4())
            title     String  @db.VarChar(200)
            published Boolean @default(false)
            author    User    @relation(fields: [authorId], references: [id])
            authorId  String  @map("author_id")
        }
    "#;
    parse("conformance", source).unwrap()
}

fn setup_sql(dialect: &dyn DbDialect, schema: &Schema) -> String {
    let mut stmts = Vec::new();

    // PostgreSQL needs enum types before tables; SQLite is a no-op.
    for e in schema.enums.values() {
        for s in dialect.create_enum(e) {
            stmts.push(s.sql);
        }
    }

    for m in schema.models.values() {
        for s in full_create_table(dialect, schema, m) {
            stmts.push(s.sql);
        }
    }

    stmts.join("\n")
}

fn dialect_for_backend(db: &TestDb) -> &'static dyn DbDialect {
    match db.backend().as_str() {
        "postgres" => dialect_for(ruprizzle_core::ir::Provider::Postgres),
        "sqlite" => dialect_for(ruprizzle_core::ir::Provider::Sqlite),
        _ => unreachable!(),
    }
}

fn schema_for_backend(_db: &TestDb) -> Schema {
    conformance_schema()
}

both_dbs! {
    setup = "";
    async fn create_tables_and_insert(db: TestDb) {
        let schema = schema_for_backend(&db);
        let dialect = dialect_for_backend(&db);
        db.execute(&setup_sql(dialect, &schema)).await?;

        let user_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11";
        let post_id = "b0eebc99-9c0b-4ef8-bb6d-6bb9bd380a22";

        db.execute(&format!(
            r#"INSERT INTO "users" (id, email, name, is_admin) VALUES ('{user_id}', 'alice@example.com', 'Alice', false)"#
        )).await?;

        db.execute(&format!(
            r#"INSERT INTO "posts" (id, title, published, author_id) VALUES ('{post_id}', 'Hello', true, '{user_id}')"#
        )).await?;

        let count = db.fetch_i64(r#"SELECT count(*) FROM "users""#).await?;
        assert_eq!(count, 1);

        let title = db.fetch_string(r#"SELECT title FROM "posts""#).await?;
        assert_eq!(title, "Hello");

        let published = db.fetch_i64(r#"SELECT count(*) FROM "posts" WHERE published = true"#).await?;
        assert_eq!(published, 1);
    }
}

both_dbs! {
    setup = "";
    async fn unique_and_foreign_key_constraints(db: TestDb) {
        let schema = schema_for_backend(&db);
        let dialect = dialect_for_backend(&db);
        db.execute(&setup_sql(dialect, &schema)).await?;

        let user_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11";

        db.execute(&format!(
            r#"INSERT INTO "users" (id, email, name, is_admin) VALUES ('{user_id}', 'alice@example.com', 'Alice', false)"#
        )).await?;

        // Duplicate email must fail.
        let result = db.execute(
            r#"INSERT INTO "users" (id, email, name, is_admin) VALUES ('c0eebc99-9c0b-4ef8-bb6d-6bb9bd380a33', 'alice@example.com', 'Bob', false)"#
        ).await;
        assert!(result.is_err(), "unique constraint should reject duplicates");

        // Referencing a missing user must fail.
        let result = db.execute(r#"
            INSERT INTO "posts" (id, title, published, author_id)
            VALUES ('d0eebc99-9c0b-4ef8-bb6d-6bb9bd380a44', 'Orphan', false, '00000000-0000-0000-0000-000000000000')
        "#).await;
        assert!(result.is_err(), "foreign key should reject missing target");
    }
}

both_dbs! {
    setup = "";
    async fn alter_column_rename_preserves_data(db: TestDb) {
        let schema = schema_for_backend(&db);
        let dialect = dialect_for_backend(&db);
        db.execute(&setup_sql(dialect, &schema)).await?;

        let user_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11";

        db.execute(&format!(
            r#"INSERT INTO "users" (id, email, name, is_admin) VALUES ('{user_id}', 'alice@example.com', 'Alice', false)"#
        )).await?;

        let user = schema.model("User").unwrap();
        let email = user.field("email").unwrap();
        let mut renamed = email.clone();
        renamed.column = "mail".to_owned();

        let stmts = full_alter_column(dialect, &schema, user, email, &renamed);
        let sql = stmts.iter().map(|s| s.sql.as_str()).collect::<Vec<_>>().join("\n");
        db.execute(&sql).await?;

        let count = db.fetch_i64(r#"SELECT count(*) FROM "users""#).await?;
        assert_eq!(count, 1);

        let mail = db.fetch_string(r#"SELECT "mail" FROM "users""#).await?;
        assert_eq!(mail, "alice@example.com");
    }
}
