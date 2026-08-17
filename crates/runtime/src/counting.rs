//! An [`Executor`] wrapper that counts the statements sent to the database.
//!
//! The batched relation loader promises a bounded number of queries: one per
//! relation *level*, never one per row. A promise like that is only real if it
//! is asserted, and it can only be asserted from outside the query builder —
//! hence this wrapper, which any test can put in front of a pool:
//!
//! ```ignore
//! let counter = CountingExecutor::new(&pool);
//! let users = SelectQuery::<User>::new(&counter)
//!     .include(user_posts().include(post_comments()))
//!     .fetch_all()
//!     .await?;
//! assert_eq!(counter.count(), 3); // users, posts, comments
//! ```
//!
//! It is deliberately in the library rather than the test harness: an
//! application with its own hot path has the same question to ask.

use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};

use ruprizzle_dialect::DbDialect;

use crate::BoxFuture;
use crate::error::Error;
use crate::executor::{BoxRowStream, Executor};
use crate::value::Value;

/// Wraps an executor and counts every statement that reaches it.
pub struct CountingExecutor<'e> {
    inner: &'e dyn Executor,
    count: AtomicUsize,
}

impl std::fmt::Debug for CountingExecutor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountingExecutor")
            .field("count", &self.count())
            .finish_non_exhaustive()
    }
}

impl<'e> CountingExecutor<'e> {
    /// Wraps `inner`, starting from zero.
    #[must_use]
    pub fn new(inner: &'e dyn Executor) -> Self {
        Self {
            inner,
            count: AtomicUsize::new(0),
        }
    }

    /// How many statements have been run through this wrapper.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Resets the counter to zero, so one wrapper can bracket several
    /// measurements.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }

    fn tick(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

impl Executor for CountingExecutor<'_> {
    fn dialect(&self) -> &dyn DbDialect {
        self.inner.dialect()
    }

    fn full_table_include_limit(&self) -> u64 {
        self.inner.full_table_include_limit()
    }

    #[cfg(feature = "sqlite-rusqlite")]
    fn as_rusqlite(&self) -> Option<&crate::rusqlite::RusqlitePool> {
        self.inner.as_rusqlite()
    }

    fn on_query(&self) {
        self.tick();
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<crate::executor::RowBatch, Error>> {
        self.tick();
        self.inner.fetch_all_raw(sql, binds)
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, Error>> {
        self.tick();
        self.inner.execute_raw(sql, binds)
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        self.tick();
        self.inner.stream_raw(sql, binds)
    }

    fn stream_unbuffered_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        self.tick();
        self.inner.stream_unbuffered_raw(sql, binds)
    }
}
