//! Query manifest recording for offline validation.

use ruprizzle::{Column, Executor, Model, Pool, SelectQuery, connect};

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
}

impl Model for Task {
    const TABLE: &'static str = "manifest_tasks";
}

const NAME: Column<Task, String> = Column::new("manifest_tasks", "name");

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
