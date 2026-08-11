pub mod _generated;
pub mod enums;
pub mod bench_row;
pub mod bench_bulk;
pub mod bench_parent;
pub mod bench_child;
pub mod bench_grand_child;
pub use _generated::{RUPRIZZLE_VERSION, SCHEMA_HASH};
pub use self::bench_row::{BenchRowInsert, BenchRow, BenchRowRepo, BenchRowUpdate};
pub use self::bench_bulk::{BenchBulkInsert, BenchBulk, BenchBulkRepo, BenchBulkUpdate};
pub use self::bench_parent::{
    BenchParentInsert, BenchParent, BenchParentRepo, BenchParentUpdate,
};
pub use self::bench_child::{
    BenchChildInsert, BenchChild, BenchChildRepo, BenchChildUpdate,
};
pub use self::bench_grand_child::{
    BenchGrandChildInsert, BenchGrandChild, BenchGrandChildRepo, BenchGrandChildUpdate,
};
/// The generated `Db` client root.
#[derive(Debug, Clone)]
pub struct Db {
    pool: ::ruprizzle::Pool,
}
impl Db {
    /// Connect to a database by URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL cannot be parsed or the connection fails.
    pub async fn connect(url: &str) -> Result<Self, ::ruprizzle::Error> {
        let pool = ::ruprizzle::connect(url).await?;
        Ok(Self { pool })
    }
    /// Wrap an existing pool.
    pub fn from_pool(pool: ::ruprizzle::Pool) -> Self {
        Self { pool }
    }
    /// Return the raw `sqlx` pool.
    pub fn raw_pool(&self) -> &::ruprizzle::Pool {
        &self.pool
    }
    /// Entry point for this model's repository.
    pub fn bench_row(&self) -> BenchRowRepo<'_> {
        BenchRowRepo::new(self)
    }
    /// Entry point for this model's repository.
    pub fn bench_bulk(&self) -> BenchBulkRepo<'_> {
        BenchBulkRepo::new(self)
    }
    /// Entry point for this model's repository.
    pub fn bench_parent(&self) -> BenchParentRepo<'_> {
        BenchParentRepo::new(self)
    }
    /// Entry point for this model's repository.
    pub fn bench_child(&self) -> BenchChildRepo<'_> {
        BenchChildRepo::new(self)
    }
    /// Entry point for this model's repository.
    pub fn bench_grand_child(&self) -> BenchGrandChildRepo<'_> {
        BenchGrandChildRepo::new(self)
    }
    /// Drizzle-flavoured `SELECT` entry point.
    pub fn select<M: ::ruprizzle::Model>(&self) -> ::ruprizzle::SelectQuery<'_, M> {
        ::ruprizzle::SelectQuery::new(self.raw_pool())
    }
    /// Drizzle-flavoured `INSERT` entry point.
    pub fn insert<M: ::ruprizzle::Model>(&self) -> ::ruprizzle::InsertQuery<'_, M> {
        ::ruprizzle::InsertQuery::new(self.raw_pool())
    }
    /// Run a closure inside a transaction.
    ///
    /// The closure receives a `&mut ::ruprizzle::Tx` and can execute raw
    /// SQL through it. The transaction is committed if the closure returns
    /// `Ok`, and rolled back if it returns `Err`.
    pub async fn transaction<F, T>(&self, f: F) -> Result<T, ::ruprizzle::Error>
    where
        F: for<'t> FnOnce(
            &'t mut ::ruprizzle::Tx,
        ) -> ::ruprizzle::BoxFuture<'t, Result<T, ::ruprizzle::Error>>,
    {
        let mut tx = ::ruprizzle::Tx::begin(self.raw_pool()).await?;
        match f(&mut tx).await {
            Ok(v) => {
                tx.commit().await?;
                Ok(v)
            }
            Err(e) => {
                tx.rollback().await?;
                Err(e)
            }
        }
    }
    /// Run a closure inside a transaction with an explicit isolation level.
    pub async fn transaction_with<F, T>(
        &self,
        level: ::ruprizzle::IsolationLevel,
        f: F,
    ) -> Result<T, ::ruprizzle::Error>
    where
        F: for<'t> FnOnce(
            &'t mut ::ruprizzle::Tx,
        ) -> ::ruprizzle::BoxFuture<'t, Result<T, ::ruprizzle::Error>>,
    {
        let mut tx = ::ruprizzle::Tx::begin_with_isolation(self.raw_pool(), level)
            .await?;
        match f(&mut tx).await {
            Ok(v) => {
                tx.commit().await?;
                Ok(v)
            }
            Err(e) => {
                tx.rollback().await?;
                Err(e)
            }
        }
    }
    /// Run a transaction, retrying transient serialization failures.
    ///
    /// Retries only errors [`ruprizzle::is_retryable`] recognises —
    /// Postgres `40001`/`40P01` and SQLite lock contention. A genuine
    /// constraint violation is returned immediately rather than
    /// retried, because retrying it only repeats the work before
    /// failing the same way. `attempts` counts total tries, so
    /// `attempts = 1` disables retrying.
    ///
    /// The closure is `FnMut` because it runs once per attempt.
    pub async fn transaction_retrying<F, T>(
        &self,
        attempts: u32,
        mut f: F,
    ) -> Result<T, ::ruprizzle::Error>
    where
        F: for<'t> FnMut(
            &'t mut ::ruprizzle::Tx,
        ) -> ::ruprizzle::BoxFuture<'t, Result<T, ::ruprizzle::Error>>,
    {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut tx = ::ruprizzle::Tx::begin(self.raw_pool()).await?;
            let result = f(&mut tx).await;
            match result {
                Ok(v) => {
                    match tx.commit().await {
                        Ok(()) => return Ok(v),
                        Err(e) => {
                            if attempt >= attempts.max(1)
                                || !::ruprizzle::is_retryable(&e)
                            {
                                return Err(e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tx.rollback().await?;
                    if attempt >= attempts.max(1) || !::ruprizzle::is_retryable(&e) {
                        return Err(e);
                    }
                }
            }
        }
    }
}
/// Common imports for this generated module.
pub mod prelude {
    pub use super::{
        Db, RUPRIZZLE_VERSION, SCHEMA_HASH, BenchRow, BenchRowInsert, BenchRowUpdate,
        BenchRowRepo, bench_row, BenchBulk, BenchBulkInsert, BenchBulkUpdate,
        BenchBulkRepo, bench_bulk, BenchParent, BenchParentInsert, BenchParentUpdate,
        BenchParentRepo, bench_parent, BenchChild, BenchChildInsert, BenchChildUpdate,
        BenchChildRepo, bench_child, BenchGrandChild, BenchGrandChildInsert,
        BenchGrandChildUpdate, BenchGrandChildRepo, bench_grand_child,
    };
    pub use ::ruprizzle::prelude::*;
}