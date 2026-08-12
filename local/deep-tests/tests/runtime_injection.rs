//! SQL-injection adversarial tests against a local SQLite file.
//!
//! The query builder must never interpolate user data. These tests pass strings
//! that would be dangerous if interpolated, then assert that only the exact row
//! matches and that the compiled SQL still uses parameter placeholders.

use ruprizzle::{Column, Executor, InsertQuery, Model, SelectQuery, Value};
use ruprizzle_deep_tests::fresh_pool;

#[derive(Debug, Clone, Default, sqlx::FromRow, PartialEq)]
struct Note {
    id: i64,
    body: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Note);

impl Model for Note {
    const TABLE: &'static str = "notes";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Note {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            body: row.take::<String>(1)?,
        })
    }
}

const ID: Column<Note, i64> = Column::new("notes", "id");
const BODY: Column<Note, String> = Column::new("notes", "body");

const MALICIOUS: &[&str] = &[
    "'; DROP TABLE notes; --",
    "1 OR 1=1",
    "normal' OR 'x'='x",
    "hello; DELETE FROM notes;",
    "%_%",
    "\"quoted\"",
];

#[tokio::test]
async fn exact_match_survives_injection_strings() {
    let (pool, _tmp) = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    for (i, body) in MALICIOUS.iter().enumerate() {
        InsertQuery::<Note>::new(&pool)
            .set(ID, (i as i64) + 1)
            .set(BODY, *body)
            .exec()
            .await
            .unwrap();
    }

    for (i, body) in MALICIOUS.iter().enumerate() {
        let id = (i as i64) + 1;

        // Equality must return exactly this row, not all rows.
        let rows: Vec<Note> = SelectQuery::<Note>::new(&pool)
            .filter(BODY.eq(*body))
            .fetch_all()
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "exact match for body {body:?}");
        assert_eq!(rows[0].id, id);

        // Compiled SQL must still use a placeholder, not the literal.
        let compiled = SelectQuery::<Note>::new(&pool)
            .filter(BODY.eq(*body))
            .to_sql();
        assert!(
            compiled.sql.contains('?'),
            "SQL must use a placeholder: {}",
            compiled.sql
        );
        assert!(
            !compiled.sql.contains(body),
            "SQL must not contain the literal body: {}",
            compiled.sql
        );
    }
}

#[tokio::test]
async fn contains_pattern_is_bound_not_interpolated() {
    let (pool, _tmp) = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    InsertQuery::<Note>::new(&pool)
        .set(ID, 1)
        .set(BODY, "normal text")
        .exec()
        .await
        .unwrap();
    InsertQuery::<Note>::new(&pool)
        .set(ID, 2)
        .set(BODY, "'; DROP TABLE notes; --")
        .exec()
        .await
        .unwrap();

    // `contains` on a string with quote characters must still only match the row
    // whose body literally contains the substring.
    let rows: Vec<Note> = SelectQuery::<Note>::new(&pool)
        .filter(BODY.contains("'; DROP"))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 2);

    // A pattern with an underscore is still bound, not interpolated, so it is
    // treated as a LIKE wildcard. `contains("has_")` compiles to `LIKE '%has_%'`
    // and matches the row that literally contains "has" followed by any single
    // character.
    InsertQuery::<Note>::new(&pool)
        .set(ID, 3)
        .set(BODY, "has_underscore")
        .exec()
        .await
        .unwrap();

    let rows: Vec<Note> = SelectQuery::<Note>::new(&pool)
        .filter(BODY.contains("has_"))
        .fetch_all()
        .await
        .unwrap();
    assert!(
        rows.iter().any(|r| r.id == 3),
        "pattern with underscore bound as value"
    );
    assert!(
        !rows.iter().any(|r| r.id == 1),
        "did not match plain text row"
    );
}

#[tokio::test]
async fn raw_fragment_with_binds_is_safe() {
    let (pool, _tmp) = fresh_pool().await;

    pool.execute_raw(
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    InsertQuery::<Note>::new(&pool)
        .set(ID, 1)
        .set(BODY, "safe'")
        .exec()
        .await
        .unwrap();

    let raw = ruprizzle::RawFragment::new(
        vec!["body = ".to_string(), "".to_string()],
        vec![Value::Str("safe'".into())],
    );
    let compiled = SelectQuery::<Note>::new(&pool)
        .filter(ruprizzle::Filter::<Note>::raw(raw.clone()))
        .to_sql();
    assert!(compiled.sql.contains('?'));
    assert!(!compiled.sql.contains("safe'"));

    let rows: Vec<Note> = SelectQuery::<Note>::new(&pool)
        .filter(ruprizzle::Filter::<Note>::raw(raw))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].body, "safe'");
}
