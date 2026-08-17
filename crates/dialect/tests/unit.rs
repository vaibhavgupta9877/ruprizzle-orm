//! Unit tests for the dialect crate.

use ruprizzle_core::ir::{Provider, Schema};
use ruprizzle_core::{SchemaError, Span};
use ruprizzle_dialect::{
    DbDialect, JsonSupport, MySqlDialect, PostgresDialect, RustType, SqliteDialect, Stmt,
    check_schema_capabilities, dialect_for, full_alter_column, full_create_table,
};
use ruprizzle_parser::parse;

fn example_schema(name: &str) -> Schema {
    let path = format!(
        "{}/../../examples/{name}/schema.ruprizzle",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path).unwrap();
    parse(&path, &source).unwrap()
}

#[test]
fn postgres_dialect_emits_create_table() {
    let schema = example_schema("blog");
    let dialect = PostgresDialect;
    let user = schema.model("User").unwrap();

    let stmts = full_create_table(&dialect, &schema, user);
    assert!(!stmts.is_empty());
    let sql = &stmts[0].sql;
    assert!(sql.starts_with("CREATE TABLE"));
    assert!(sql.contains("\"users\""));
    assert!(sql.contains("\"id\" UUID NOT NULL PRIMARY KEY"));
    assert!(sql.contains("\"email\" TEXT NOT NULL UNIQUE"));
    assert!(sql.contains("\"created_at\" TIMESTAMPTZ NOT NULL DEFAULT NOW()"));

    // PostgreSQL emits foreign keys as separate ALTER TABLE statements.
    let post = schema.model("Post").unwrap();
    let post_stmts = full_create_table(&dialect, &schema, post);
    let fks: Vec<_> = post_stmts.iter().skip(1).collect();
    assert!(!fks.is_empty());
    assert!(fks[0].sql.starts_with("ALTER TABLE"));
}

#[test]
fn sqlite_dialect_emits_create_table_with_inline_fks() {
    let schema = example_schema("saas-tenant");
    let dialect = SqliteDialect;
    let membership = schema.model("Membership").unwrap();

    let stmts = full_create_table(&dialect, &schema, membership);
    assert_eq!(stmts.len(), 1);
    let sql = &stmts[0].sql;
    assert!(sql.starts_with("CREATE TABLE"));
    assert!(sql.contains("`memberships`"));
    assert!(sql.contains("`org_id` TEXT NOT NULL"));
    assert!(sql.contains("FOREIGN KEY"));
    assert!(sql.contains("REFERENCES"));
}

#[test]
fn sqlite_dialect_emulates_enum_as_text_with_check() {
    let schema = example_schema("saas-tenant");
    let dialect = SqliteDialect;
    let membership = schema.model("Membership").unwrap();

    let stmts = full_create_table(&dialect, &schema, membership);
    let sql = &stmts[0].sql;
    assert!(sql.contains("`is_owner` INTEGER NOT NULL DEFAULT 0"));
}

#[test]
fn type_matrix_matches_plan() {
    let pg = PostgresDialect;
    let sqlite = SqliteDialect;

    let schema = example_schema("blog");
    let post = schema.model("Post").unwrap();
    let title = post.field("title").unwrap();
    let published = post.field("published").unwrap();

    assert_eq!(pg.column_type(title).unwrap(), "VARCHAR(200)");
    assert_eq!(sqlite.column_type(title).unwrap(), "TEXT");
    assert_eq!(pg.column_type(published).unwrap(), "BOOLEAN");
    assert_eq!(sqlite.column_type(published).unwrap(), "INTEGER");
}

#[test]
fn rust_types_match_scalar_kinds() {
    let pg = PostgresDialect;
    let schema = example_schema("blog");
    let user = schema.model("User").unwrap();

    assert!(matches!(
        pg.rust_type(user.field("id").unwrap()),
        RustType::Uuid
    ));
    assert!(matches!(
        pg.rust_type(user.field("email").unwrap()),
        RustType::String
    ));
    assert!(
        matches!(
            pg.rust_type(user.field("name").unwrap()),
            RustType::Option(_)
        ),
        "optional fields wrap RustType::Option"
    );
}

#[test]
fn sqlite_capabilities_match_plan() {
    let cap = SqliteDialect.capabilities();
    assert!(!cap.native_enums);
    assert!(!cap.native_uuid);
    assert!(!cap.alter_column_type);
    assert!(cap.returning);
    assert!(matches!(cap.json_type, JsonSupport::TextEncoded));
}

#[test]
fn postgres_capabilities_match_plan() {
    let cap = PostgresDialect.capabilities();
    assert!(cap.native_enums);
    assert!(cap.native_uuid);
    assert!(cap.alter_column_type);
    assert!(cap.returning);
    assert!(matches!(cap.json_type, JsonSupport::Native));
}

#[test]
fn mysql_dialect_emits_mysql_ddl_and_dml_fragments() {
    let mysql = MySqlDialect;
    let schema = example_schema("blog");
    let user = schema.model("User").unwrap();

    assert_eq!(mysql.name(), "mysql");
    assert_eq!(mysql.quote_ident("users`archive"), "`users``archive`");
    assert_eq!(
        mysql.column_type(user.field("id").unwrap()).unwrap(),
        "CHAR(36)"
    );
    assert_eq!(
        mysql.column_type(user.field("createdAt").unwrap()).unwrap(),
        "DATETIME(6)"
    );

    let post = schema.model("Post").unwrap();
    let stmts = full_create_table(&mysql, &schema, post);
    assert!(stmts[0].sql.contains("VARCHAR(255)"));
    assert!(stmts.iter().any(|stmt| stmt.sql.contains("ADD CONSTRAINT")));
    assert_eq!(
        mysql.upsert_clause(&["email".to_owned()], &[]),
        "ON DUPLICATE KEY UPDATE `email` = `email`"
    );
    assert!(!mysql.capabilities().returning);
    assert!(!mysql.capabilities().deferrable_fks);
}

#[test]
fn mysql_provider_selects_mysql_dialect() {
    assert_eq!(Provider::parse("mysql"), Some(Provider::Mysql));
    assert_eq!(Provider::parse("mariadb"), Some(Provider::Mysql));
    assert_eq!(dialect_for(Provider::Mysql).name(), "mysql");
}

#[test]
fn postgres_upsert_clause() {
    let pg = PostgresDialect;
    assert_eq!(
        pg.upsert_clause(&["email".to_owned()], &[]),
        "ON CONFLICT (email) DO NOTHING"
    );
    assert_eq!(
        pg.upsert_clause(&["email".to_owned()], &["name".to_owned()]),
        r#"ON CONFLICT (email) DO UPDATE SET "name" = EXCLUDED."name""#
    );
}

#[test]
fn sqlite_upsert_clause() {
    let sqlite = SqliteDialect;
    assert_eq!(
        sqlite.upsert_clause(&["email".to_owned()], &[]),
        "ON CONFLICT (email) DO NOTHING"
    );
}

#[test]
fn limit_offset_renders() {
    let pg = PostgresDialect;
    assert_eq!(pg.limit_offset(Some(10), Some(20)), "LIMIT 10 OFFSET 20");
    assert_eq!(pg.limit_offset(None, Some(20)), "OFFSET 20");
    assert_eq!(pg.limit_offset(Some(10), None), "LIMIT 10");
}

#[test]
fn check_schema_capabilities_warns_on_sqlite_decimal_and_json() {
    let source = r#"
        datasource db {
            provider = "sqlite"
            url      = env("DATABASE_URL")
        }

        generator client {
            provider = "prisma-client-js"
        }

        model Product {
            id          String  @id @default(uuid4())
            price       Decimal
            settings    Json?
            is_active   Boolean
        }
    "#;
    let schema = parse("test", source).unwrap();
    let dialect = SqliteDialect;
    let warnings = check_schema_capabilities(&dialect, &schema);

    let has_decimal = warnings.iter().any(|w| match w {
        SchemaError::DialectDegraded { construct, .. } => construct == "Decimal",
        _ => false,
    });
    let has_json = warnings.iter().any(|w| match w {
        SchemaError::DialectDegraded { construct, .. } => construct == "Json",
        _ => false,
    });

    assert!(has_decimal, "Decimal on SQLite should warn");
    assert!(has_json, "Json on SQLite should warn");
}

#[test]
fn object_safety_compiles() {
    fn takes_dyn(d: &dyn DbDialect) -> Vec<Stmt> {
        d.drop_table("users")
    }

    let pg = PostgresDialect;
    let sqlite = SqliteDialect;
    assert_eq!(takes_dyn(&pg).len(), 1);
    assert_eq!(takes_dyn(&sqlite).len(), 1);
}

#[test]
fn dialect_for_selects_provider() {
    assert_eq!(dialect_for(Provider::Postgres).name(), "postgres");
    assert_eq!(dialect_for(Provider::Sqlite).name(), "sqlite");
}

#[test]
fn sqlite_rebuild_table_preserves_other_columns() {
    let schema = example_schema("saas-tenant");
    let dialect = SqliteDialect;
    let membership = schema.model("Membership").unwrap();
    let is_owner = membership.field("isOwner").unwrap();
    let mut new_field = is_owner.clone();
    new_field.column = "is_admin".to_owned();

    let stmts = full_alter_column(&dialect, &schema, membership, is_owner, &new_field);
    let sql = stmts
        .iter()
        .map(|s| s.sql.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(sql.contains("PRAGMA foreign_keys=OFF"));
    assert!(sql.contains("CREATE TABLE `memberships__new`"));
    assert!(sql.contains("INSERT INTO `memberships__new`"));
    assert!(sql.contains("DROP TABLE `memberships`"));
    assert!(sql.contains("ALTER TABLE `memberships__new` RENAME TO `memberships`"));
    assert!(sql.contains("PRAGMA foreign_key_check"));
}

#[test]
fn postgres_alter_column_renames() {
    let schema = example_schema("blog");
    let dialect = PostgresDialect;
    let user = schema.model("User").unwrap();
    let email = user.field("email").unwrap();
    let mut new_field = email.clone();
    new_field.column = "mail".to_owned();

    let stmts = full_alter_column(&dialect, &schema, user, email, &new_field);
    let sql = stmts
        .iter()
        .map(|s| s.sql.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(sql.contains("RENAME COLUMN"));
    assert!(sql.contains("\"mail\""));
}

#[test]
fn create_enum_on_postgres_and_noop_on_sqlite() {
    let schema = example_schema("blog");
    let role = schema.enum_def("Role").unwrap();

    let pg = PostgresDialect;
    let sqlite = SqliteDialect;

    let pg_stmts = pg.create_enum(role);
    assert_eq!(pg_stmts.len(), 1);
    assert!(pg_stmts[0].sql.starts_with("CREATE TYPE"));

    let sqlite_stmts = sqlite.create_enum(role);
    assert!(sqlite_stmts.is_empty());
}

#[test]
fn invalid_native_type_produces_error() {
    let mut schema = example_schema("blog");
    let user = schema.models.get_mut("User").unwrap();
    let email = user.fields.get_mut("email").unwrap();
    email.attrs.native_type = Some(ruprizzle_core::ir::NativeType {
        name: "Blob".to_owned(),
        args: Vec::new(),
        span: Span::EMPTY,
    });

    let pg = PostgresDialect;
    let result = pg.column_type(email);
    assert!(result.is_err());
}

#[test]
fn postgres_renders_partial_expression_indexes_and_generated_columns() {
    let source = r#"
        datasource db {
            provider   = "postgres"
            url        = env("DATABASE_URL")
            extensions = ["uuid-ossp"]
        }

        model User {
            id         Uuid    @id @default(uuid7())
            first_name String
            last_name  String
            full_name  String? @generated("always as (first_name || ' ' || last_name) stored")

            @@index([first_name, last_name], where: "last_name IS NOT NULL")
            @@unique(["(coalesce(first_name, ''))"])
        }
    "#;
    let schema = parse("test", source).unwrap();
    let dialect = PostgresDialect;
    let user = schema.model("User").unwrap();

    let table = &dialect.create_table(&schema, user)[0].sql;
    assert!(table.contains("GENERATED ALWAYS AS (first_name || ' ' || last_name) STORED"));

    let index = &dialect.create_index(user, &user.indexes[0])[0].sql;
    assert!(index.contains("CREATE INDEX"));
    assert!(index.contains("WHERE last_name IS NOT NULL"));

    let unique = &dialect.add_unique(user, &user.uniques[0])[0].sql;
    assert!(unique.contains("(coalesce(first_name, ''))"));
}
