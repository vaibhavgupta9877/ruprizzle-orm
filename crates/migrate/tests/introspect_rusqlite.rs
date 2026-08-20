//! `db pull` introspection over the native `rusqlite` driver.
//!
//! Introspection decodes rows through a driver-specific path (`rusqlite_cells`)
//! that no test previously exercised, so a break there was invisible. This test
//! pulls a real schema through the native driver end to end.

#![cfg(feature = "sqlite-rusqlite")]

use std::borrow::Cow;

use ruprizzle::Executor;

async fn rusqlite_pool(dir: &tempfile::TempDir) -> ruprizzle::Pool {
    let path = dir.path().join("introspect.sqlite");
    let file = path
        .display()
        .to_string()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");
    ruprizzle::connect(&url).await.unwrap()
}

#[tokio::test]
async fn pulls_tables_columns_and_foreign_keys() {
    let dir = tempfile::tempdir().unwrap();
    let pool = rusqlite_pool(&dir).await;

    for ddl in [
        "CREATE TABLE authors (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        "CREATE TABLE books (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            pages INTEGER,
            author_id INTEGER NOT NULL REFERENCES authors(id)
        )",
        "CREATE INDEX books_title_idx ON books (title)",
    ] {
        pool.execute_raw(Cow::Owned(ddl.to_owned()), Vec::new())
            .await
            .unwrap();
    }

    let schema = ruprizzle_migrate::introspect::pull(&pool).await.unwrap();

    let mut names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["authors", "books"]);

    let books = schema
        .tables
        .iter()
        .find(|t| t.name == "books")
        .expect("books table");

    let mut columns: Vec<&str> = books.columns.iter().map(|c| c.name.as_str()).collect();
    columns.sort_unstable();
    assert_eq!(columns, ["author_id", "id", "pages", "title"]);

    let title = books
        .columns
        .iter()
        .find(|c| c.name == "title")
        .expect("title column");
    assert!(!title.nullable, "title is declared NOT NULL");

    let pages = books
        .columns
        .iter()
        .find(|c| c.name == "pages")
        .expect("pages column");
    assert!(pages.nullable, "pages has no NOT NULL constraint");

    assert_eq!(books.foreign_keys.len(), 1, "books -> authors");
    let fk = &books.foreign_keys[0];
    assert_eq!(fk.target_table, "authors");
    assert_eq!(fk.columns, ["author_id"]);

    assert!(
        books.indexes.iter().any(|i| i.name == "books_title_idx"),
        "expected books_title_idx, got {:?}",
        books.indexes
    );
}
