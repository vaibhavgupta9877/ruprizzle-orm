//! Query manifest recording for offline validation.

use ruprizzle::{Column, Executor, Model, Pool, SelectQuery, connect};

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Task);

impl Model for Task {
    const TABLE: &'static str = "manifest_tasks";
}

const NAME: Column<Task, String> = Column::new("manifest_tasks", "name");

// The `Model` bound pulls in the native-driver decode traits when the
// `sqlite-rusqlite` feature is on. This hand-written model does not go through
// `#[derive(Model)]`, so the impls are supplied here.
#[cfg(feature = "sqlite-rusqlite")]
mod rusqlite_impls {
    use super::Task;
    use ruprizzle::Error;
    use ruprizzle::rusqlite::{FromOwnedRow, FromRusqliteRow, Row, RusqliteRow, RusqliteValue};

    impl FromOwnedRow for Task {
        fn from_owned_row(row: &Row) -> Result<Self, Error> {
            let id = match row.values.first() {
                Some(RusqliteValue::Integer(id)) => *id,
                _ => 0,
            };
            let name = match row.values.get(1) {
                Some(RusqliteValue::Text(name)) => name.clone(),
                _ => String::new(),
            };
            Ok(Task { id, name })
        }
    }

    impl FromRusqliteRow for Task {
        fn from_rusqlite_row(row: &RusqliteRow) -> Result<Self, Error> {
            Ok(Task {
                id: row
                    .get(0)
                    .map_err(|e| Error::Message(format!("cannot decode Task.id: {e}")))?,
                name: row
                    .get(1)
                    .map_err(|e| Error::Message(format!("cannot decode Task.name: {e}")))?,
            })
        }
    }
}

async fn fresh_pool() -> Pool {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let url = format!("sqlite:///{}?mode=rwc", file);
    let pool = connect(&url).await.unwrap();

    pool.execute_raw(
        "CREATE TABLE manifest_tasks (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
            .to_string()
            .into(),
        Vec::new(),
    )
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn records_to_sql_output_when_enabled() {
    unsafe {
        std::env::set_var("RUPRIZZLE_RECORD_QUERIES", "1");
    }
    ruprizzle::query_manifest::clear();

    let pool = fresh_pool().await;
    let _ = SelectQuery::<Task>::new(&pool)
        .filter(NAME.eq("x"))
        .to_sql()
        .unwrap();

    let file = tempfile::NamedTempFile::new().unwrap();
    ruprizzle::query_manifest::write_manifest(file.path()).unwrap();

    let source = std::fs::read_to_string(file.path()).unwrap();
    let manifest: ruprizzle_check::QueryManifest = serde_json::from_str(&source).unwrap();
    assert_eq!(manifest.queries.len(), 1);
    assert!(manifest.queries[0].sql.contains("manifest_tasks"));
    assert_eq!(manifest.queries[0].dialect, "sqlite");

    unsafe {
        std::env::remove_var("RUPRIZZLE_RECORD_QUERIES");
    }
    ruprizzle::query_manifest::clear();
}
