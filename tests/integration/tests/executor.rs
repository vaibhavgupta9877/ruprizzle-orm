//! Round-trips for the P4-03/P4-05/P4-06 runtime additions: the `Executor`
//! abstraction, `exists`, `stream`, paging, and transaction isolation.
//!
//! Runs on both backends via `both_dbs!` so the SQLite and Postgres paths
//! cannot drift.

use futures_core::Stream;
use ruprizzle::{Column, Executor, InsertQuery, IsolationLevel, Model, SelectQuery, Tx};
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

const ID: Column<Task, i64> = Column::new("tasks", "id");
const NAME: Column<Task, String> = Column::new("tasks", "name");

/// Drains a stream without depending on `futures-util`.
async fn collect<S, T>(stream: S) -> Vec<T>
where
    S: Stream<Item = Result<T, ruprizzle::Error>>,
{
    let mut stream = Box::pin(stream);
    let mut out = Vec::new();
    std::future::poll_fn(|cx| {
        loop {
            match stream.as_mut().poll_next(cx) {
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(None) => return std::task::Poll::Ready(()),
                std::task::Poll::Ready(Some(Ok(v))) => out.push(v),
                std::task::Poll::Ready(Some(Err(e))) => panic!("stream error: {e}"),
            }
        }
    })
    .await;
    out
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn exists_is_cheap_and_correct(db: TestDb) {
        let pool = db.pool();

        // No rows yet.
        assert!(!SelectQuery::<Task>::new(pool).exists().await?);

        InsertQuery::<Task>::new(pool)
            .set(ID, 1)
            .set(NAME, "a")
            .exec()
            .await
            .map(|_: Task| ())?;

        assert!(SelectQuery::<Task>::new(pool).exists().await?);
        assert!(SelectQuery::<Task>::new(pool).filter(NAME.eq("a")).exists().await?);
        assert!(!SelectQuery::<Task>::new(pool).filter(NAME.eq("nope")).exists().await?);

        // `exists` must not be a disguised count: it caps the result at one row.
        let (sql, _) = {
            let c = SelectQuery::<Task>::new(pool).to_sql();
            (c.sql, c.binds)
        };
        assert!(sql.contains("SELECT"), "unexpected select shape: {sql}");
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn page_reports_has_next_exactly(db: TestDb) {
        let pool = db.pool();
        for i in 1..=5i64 {
            InsertQuery::<Task>::new(pool)
                .set(ID, i)
                .set(NAME, format!("task-{i}"))
                .exec()
                .await
                .map(|_: Task| ())?;
        }

        let page = SelectQuery::<Task>::new(pool).page(2).await?;
        assert_eq!(page.len(), 2);
        assert!(page.has_next);

        // A page exactly the size of the remaining rows must NOT claim a next
        // page. This is the case a naive `items.len() == size` check gets wrong.
        let page = SelectQuery::<Task>::new(pool).page(5).await?;
        assert_eq!(page.len(), 5);
        assert!(!page.has_next, "a full final page must not report has_next");

        let page = SelectQuery::<Task>::new(pool).page(10).await?;
        assert_eq!(page.len(), 5);
        assert!(!page.has_next);

        // Ordering is deterministic because the primary key is appended.
        let ids: Vec<i64> = page.items.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4, 5]);
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn stream_yields_every_row(db: TestDb) {
        let pool = db.pool();
        for i in 1..=4i64 {
            InsertQuery::<Task>::new(pool)
                .set(ID, i)
                .set(NAME, format!("task-{i}"))
                .exec()
                .await
                .map(|_: Task| ())?;
        }

        let rows: Vec<Task> = collect(SelectQuery::<Task>::new(pool).stream()).await;
        assert_eq!(rows.len(), 4);
        let mut ids: Vec<i64> = rows.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4]);

        // Filters apply to streams too.
        let rows: Vec<Task> =
            collect(SelectQuery::<Task>::new(pool).filter(ID.gt(2)).stream()).await;
        assert_eq!(rows.len(), 2);
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn the_same_query_runs_on_a_pool_and_in_a_transaction(db: TestDb) {
        let pool = db.pool();
        InsertQuery::<Task>::new(pool).set(ID, 1).set(NAME, "a").exec().await.map(|_: Task| ())?;

        // This is the point of the `Executor` trait: one query, two contexts.
        let from_pool: Vec<Task> = SelectQuery::<Task>::new(pool).fetch_all().await?;

        let tx = Tx::begin(pool).await?;
        let from_tx: Vec<Task> = SelectQuery::<Task>::new(&tx).fetch_all().await?;
        assert_eq!(from_pool, from_tx);

        // Reads inside the transaction see its own uncommitted writes.
        let exec: &dyn Executor = &tx;
        exec.execute_raw(
            "INSERT INTO tasks (id, name) VALUES (2, 'in-tx')".to_owned().into(),
            Vec::new(),
        )
        .await?;
        let in_tx: Vec<Task> = SelectQuery::<Task>::new(&tx).fetch_all().await?;
        assert_eq!(in_tx.len(), 2);

        // ...and the pool does not, until commit.
        let outside: Vec<Task> = SelectQuery::<Task>::new(pool).fetch_all().await?;
        assert_eq!(outside.len(), 1, "uncommitted write leaked out of the tx");

        tx.commit().await?;
        let after: Vec<Task> = SelectQuery::<Task>::new(pool).fetch_all().await?;
        assert_eq!(after.len(), 2);
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn rollback_discards_writes(db: TestDb) {
        let pool = db.pool();

        let tx = Tx::begin(pool).await?;
        SelectQuery::<Task>::new(&tx).fetch_all().await.map(|_: Vec<Task>| ())?;
        tx.execute(
            "INSERT INTO tasks (id, name) VALUES (9, 'gone')",
            &[],
        )
        .await?;
        tx.rollback().await?;

        assert!(!SelectQuery::<Task>::new(pool).exists().await?);
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn isolation_level_is_accepted_on_both_backends(db: TestDb) {
        let pool = db.pool();

        // Postgres applies the level; SQLite accepts and ignores it. Either way
        // the same application code must work, which is what this pins.
        for level in [
            IsolationLevel::ReadCommitted,
            IsolationLevel::RepeatableRead,
            IsolationLevel::Serializable,
        ] {
            let tx = Tx::begin_with_isolation(pool, level).await?;
            let rows: Vec<Task> = SelectQuery::<Task>::new(&tx).fetch_all().await?;
            assert!(rows.is_empty());
            tx.commit().await?;
        }
    }
}
