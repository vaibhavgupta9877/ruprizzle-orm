//! Transaction lifecycle guarantees that must hold on every SQLite and Postgres
//! backend, including the native `rusqlite` pool where a transaction owns a
//! physical connection for its whole lifetime.
//!
//! Runs on both backends via `both_dbs!` so the drivers cannot drift.

use ruprizzle::{Executor, Model, SelectQuery, Tx};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq, Default, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Task);

impl Model for Task {
    const TABLE: &'static str = "tasks";
    const PRIMARY_KEY: &'static str = "id";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Task {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn dropped_transactions_return_their_connection(db: TestDb) {
        let pool = db.pool();

        // The testkit pools hold four connections. If an abandoned transaction
        // leaked its connection, eight abandoned transactions would exhaust the
        // pool and the statements below would fail with PoolExhausted. Each
        // iteration abandons its transaction (no commit, no rollback) so the
        // `Drop` path is the only thing returning the connection.
        for id in 0..8_i64 {
            let tx = Tx::begin(pool).await?;
            tx.execute(
                &format!("INSERT INTO tasks (id, name) VALUES ({id}, 'abandoned')"),
                &[],
            )
            .await?;
            drop(tx);
        }

        // Every abandoned write was rolled back, and the pool still serves reads.
        let rows: Vec<Task> = SelectQuery::<Task>::new(pool).fetch_all().await?;
        assert!(rows.is_empty(), "abandoned transaction committed: {rows:?}");
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn pool_queries_proceed_while_a_transaction_is_open(db: TestDb) {
        let pool = db.pool();

        // An open transaction pins one connection. Other callers must still be
        // able to use the remaining connections while it is held.
        let tx = Tx::begin(pool).await?;
        tx.execute(
            "INSERT INTO tasks (id, name) VALUES (1, 'held')",
            &[],
        )
        .await?;

        let exec: &dyn Executor = pool;
        let outside = exec
            .fetch_all_raw("SELECT id FROM tasks".to_owned().into(), Vec::new())
            .await?;
        assert_eq!(outside.len(), 0, "uncommitted row visible from the pool");

        tx.commit().await?;
        let after: Vec<Task> = SelectQuery::<Task>::new(pool).fetch_all().await?;
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].name, "held");
    }
}
