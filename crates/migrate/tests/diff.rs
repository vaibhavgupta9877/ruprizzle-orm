//! Tests for the schema diff engine.

use ruprizzle_dialect::dialect_for;
use ruprizzle_migrate::{diff, down_sql, plan, up_sql};
use ruprizzle_parser::parse;

fn schema_v1() -> ruprizzle_core::ir::Schema {
    parse(
        "v1",
        r#"
datasource db {
    provider = "sqlite"
    url      = "sqlite://:memory:"
}

generator client {
    provider = "rust"
}

model User {
    id    Int    @id
    email String @unique
    name  String
}
"#,
    )
    .expect("v1 parses")
}

fn schema_v2() -> ruprizzle_core::ir::Schema {
    parse(
        "v2",
        r#"
datasource db {
    provider = "sqlite"
    url      = "sqlite://:memory:"
}

generator client {
    provider = "rust"
}

model User {
    id    Int     @id
    email String  @unique
    name  String?
    age   Int
}
"#,
    )
    .expect("v2 parses")
}

#[test]
fn diff_detects_added_column() {
    let v1 = schema_v1();
    let v2 = schema_v2();
    let changes = diff(&v1, &v2);

    let adds: Vec<_> = changes
        .iter()
        .filter_map(|c| match c {
            ruprizzle_migrate::Change::AddColumn { model, field } => {
                Some((model.as_str(), field.name.as_str()))
            }
            _ => None,
        })
        .collect();

    assert_eq!(adds, vec![("User", "age")]);
}

#[test]
fn plan_sqlite_emits_alter_table_for_added_column() {
    let v1 = schema_v1();
    let v2 = schema_v2();
    let changes = diff(&v1, &v2);

    let dialect = dialect_for(v2.datasource.provider);
    let stmts = plan(&v1, &v2, dialect, &changes);

    let sql: Vec<String> = stmts.into_iter().map(|s| s.sql).collect();
    assert!(
        sql.iter()
            .any(|s| s.contains("ADD COLUMN") && s.contains("age")),
        "expected ADD COLUMN age: {sql:?}"
    );
}

#[test]
fn diff_and_plan_is_idempotent_for_unchanged_schema() {
    let v1 = schema_v1();
    let changes = diff(&v1, &v1);
    assert!(changes.is_empty());

    let dialect = dialect_for(v1.datasource.provider);
    let stmts = plan(&v1, &v1, dialect, &changes);
    assert!(stmts.is_empty());
}

#[test]
fn down_sql_reverses_added_column() {
    let v1 = schema_v1();
    let v2 = schema_v2();
    let dialect = dialect_for(v2.datasource.provider);
    let sql = down_sql(&v1, &v2, dialect);

    assert!(
        sql.contains("DROP COLUMN") && sql.contains("age"),
        "expected down.sql to drop the age column: {sql}"
    );
    assert!(
        sql.contains("cannot restore data"),
        "expected honest irreversibility note: {sql}"
    );
}

#[test]
fn up_sql_emits_backfill_hook_for_not_null_column_without_default() {
    let v1 = schema_v1();
    let v2 = schema_v2();
    let dialect = dialect_for(v2.datasource.provider);
    let sql = up_sql(&v1, &v2, dialect);

    assert!(
        sql.contains("RUPRIZZLE:BACKFILL"),
        "expected backfill marker: {sql}"
    );
    assert!(
        sql.contains("ADD COLUMN") && sql.contains("age"),
        "expected ADD COLUMN age: {sql}"
    );
}
