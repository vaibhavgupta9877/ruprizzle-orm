//! Regression coverage for SQLite migration data and connection integrity.

use std::borrow::Cow;
use std::fs;

use ruprizzle::{Executor, Pool, RowBatch};
use ruprizzle_migrate::Migrator;
use sqlx::Row;

async fn databases() -> Vec<(tempfile::TempDir, Pool)> {
    sqlx::any::install_default_drivers();
    let mut databases = Vec::new();
    for driver in ["sqlx", "any", "rusqlite"] {
        if driver == "rusqlite" && !cfg!(feature = "sqlite-rusqlite") {
            continue;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir
            .path()
            .join("db.sqlite")
            .to_str()
            .unwrap()
            .replace('\\', "/");
        let url = format!("sqlite:///{file}?mode=rwc");
        let pool = match driver {
            "any" => Pool::Any(
                sqlx::any::AnyPoolOptions::new()
                    .max_connections(3)
                    .connect(&url)
                    .await
                    .unwrap(),
            ),
            "rusqlite" => ruprizzle::connect(&format!("{url}&driver=rusqlite"))
                .await
                .unwrap(),
            _ => ruprizzle::connect(&url).await.unwrap(),
        };
        databases.push((dir, pool));
    }
    databases
}

async fn execute(pool: &Pool, sql: &str) {
    pool.execute_raw(Cow::Owned(sql.to_owned()), vec![])
        .await
        .unwrap();
}

async fn scalar(pool: &Pool, sql: &str) -> i64 {
    let rows = pool
        .fetch_all_raw(Cow::Owned(sql.to_owned()), vec![])
        .await
        .unwrap();
    match rows {
        RowBatch::Sqlite(rows) => rows[0].get(0),
        RowBatch::Any(rows) => rows[0].get(0),
        #[cfg(feature = "sqlite-rusqlite")]
        RowBatch::Rusqlite(rows) => match &rows[0].values[0] {
            ruprizzle::rusqlite::RusqliteValue::Integer(n) => *n,
            other => panic!("expected integer: {other:?}"),
        },
        other => panic!("unexpected rows: {other:?}"),
    }
}

fn migration(dir: &std::path::Path, id: &str, up: &str, down: &str) {
    let path = dir.join(id);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("up.sql"), up).unwrap();
    fs::write(path.join("down.sql"), down).unwrap();
}

fn schema(optional: bool) -> ruprizzle_core::ir::Schema {
    ruprizzle_parser::parse(
        "test",
        &format!(
            r#"
datasource db {{ provider = "sqlite" url = "sqlite::memory:" }}
model Parent {{
  id Int @id
  label String{}
  children Child[]
  @@map("parent")
}}
model Child {{
  id Int @id
  parentId Int @map("parent_id")
  parent Parent @relation(fields: [parentId], references: [id], onDelete: Cascade)
  @@map("child")
}}
"#,
            if optional { "?" } else { "" }
        ),
    )
    .unwrap()
}

async fn initial_data(pool: &Pool) {
    execute(
        pool,
        "CREATE TABLE parent(id INTEGER PRIMARY KEY, label TEXT NOT NULL)",
    )
    .await;
    execute(pool, "CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent(id) ON DELETE CASCADE)").await;
    execute(pool, "INSERT INTO parent VALUES(1, 'keep')").await;
    execute(pool, "INSERT INTO child VALUES(1, 1)").await;
}

#[tokio::test]
async fn rebuild_preserves_children_and_enforcement() {
    for (_db, pool) in databases().await {
        initial_data(&pool).await;
        let dir = tempfile::tempdir().unwrap();
        let up = ruprizzle_migrate::up_sql(
            &schema(false),
            &schema(true),
            &ruprizzle_dialect::SqliteDialect,
        );
        assert!(up.contains("DROP TABLE"), "expected rebuild: {up}");
        migration(dir.path(), "001_rebuild", &up, "");
        Migrator::new(dir.path())
            .apply_all(&pool, true)
            .await
            .unwrap();
        assert_eq!(scalar(&pool, "SELECT count(*) FROM parent").await, 1);
        assert_eq!(scalar(&pool, "SELECT count(*) FROM child").await, 1);
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        pool.close().await;
    }
}

#[tokio::test]
async fn invalid_references_abort_migration_and_restore_enforcement() {
    for (_db, pool) in databases().await {
        initial_data(&pool).await;
        let dir = tempfile::tempdir().unwrap();
        migration(
            dir.path(),
            "001_invalid",
            "PRAGMA foreign_keys=OFF; INSERT INTO child VALUES(2, 999); PRAGMA foreign_keys=ON;",
            "",
        );
        let migrator = Migrator::new(dir.path());
        assert!(migrator.apply_all(&pool, true).await.is_err());
        assert_eq!(scalar(&pool, "SELECT count(*) FROM child").await, 1);
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        assert_eq!(migrator.pending(&pool).await.unwrap(), ["001_invalid"]);
        pool.close().await;
    }
}

#[tokio::test]
async fn reset_drops_referenced_tables_atomically() {
    for (_db, pool) in databases().await {
        execute(&pool, "CREATE TABLE parent(id INTEGER PRIMARY KEY)").await;
        execute(&pool, "CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id) ON DELETE RESTRICT)").await;
        execute(&pool, "INSERT INTO parent VALUES(1)").await;
        execute(&pool, "INSERT INTO child VALUES(1, 1)").await;
        let dir = tempfile::tempdir().unwrap();
        Migrator::new(dir.path())
            .reset(&pool, &ruprizzle_dialect::SqliteDialect)
            .await
            .unwrap();
        assert_eq!(scalar(&pool, "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('parent','child')").await, 0);
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        pool.close().await;
    }
}

#[tokio::test]
async fn abandoned_migration_restores_single_connection_memory_database() {
    sqlx::any::install_default_drivers();
    let pools = [
        Pool::Sqlite(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Pool::Any(
            sqlx::any::AnyPoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        ),
    ];
    for pool in pools {
        execute(&pool, "CREATE TABLE keep(id INTEGER PRIMARY KEY)").await;
        execute(&pool, "INSERT INTO keep VALUES(1)").await;
        let tx = ruprizzle::Tx::begin_migration(&pool).await.unwrap();
        tx.execute("INSERT INTO keep VALUES(2)", &[]).await.unwrap();
        drop(tx);
        let count = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            scalar(&pool, "SELECT count(*) FROM keep"),
        )
        .await
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        let tx = ruprizzle::Tx::begin_migration(&pool).await.unwrap();
        tx.execute("INSERT INTO keep VALUES(3)", &[]).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(scalar(&pool, "SELECT count(*) FROM keep").await, 2);
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        pool.close().await;
    }
}

#[tokio::test]
async fn failed_rollback_keeps_data_and_migration_tracking() {
    for (_db, pool) in databases().await {
        let dir = tempfile::tempdir().unwrap();
        migration(
            dir.path(),
            "001_initial",
            "CREATE TABLE rollback_items(id INTEGER PRIMARY KEY); INSERT INTO rollback_items VALUES(1);",
            "DELETE FROM rollback_items; DROP TABLE missing_table;",
        );
        let migrator = Migrator::new(dir.path());
        migrator.apply_all(&pool, false).await.unwrap();
        assert!(migrator.rollback(&pool, 1).await.is_err());
        assert_eq!(
            scalar(&pool, "SELECT count(*) FROM rollback_items").await,
            1
        );
        assert_eq!(
            migrator.status(&pool).await.unwrap().applied,
            ["001_initial"]
        );
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        pool.close().await;
    }
}

#[tokio::test]
async fn rollback_rebuild_preserves_children_and_updates_tracking() {
    for (_db, pool) in databases().await {
        initial_data(&pool).await;
        let dir = tempfile::tempdir().unwrap();
        migration(
            dir.path(),
            "001_reversible",
            "UPDATE parent SET label=label;",
            "PRAGMA foreign_keys=OFF; CREATE TABLE parent__new(id INTEGER PRIMARY KEY, label TEXT); INSERT INTO parent__new SELECT * FROM parent; DROP TABLE parent; ALTER TABLE parent__new RENAME TO parent; PRAGMA foreign_key_check; PRAGMA foreign_keys=ON;",
        );
        let migrator = Migrator::new(dir.path());
        migrator.apply_all(&pool, false).await.unwrap();
        let report = migrator.rollback(&pool, 1).await.unwrap();
        assert_eq!(report.applied, ["001_reversible"]);
        assert_eq!(scalar(&pool, "SELECT count(*) FROM child").await, 1);
        assert_eq!(scalar(&pool, "PRAGMA foreign_keys").await, 1);
        let status = migrator.status(&pool).await.unwrap();
        assert!(status.applied.is_empty());
        assert_eq!(status.pending, ["001_reversible"]);
        pool.close().await;
    }
}
