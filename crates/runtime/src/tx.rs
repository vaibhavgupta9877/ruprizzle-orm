//! Transaction handle.

use std::sync::Mutex;

use sqlx::Any;

use crate::Error;
use crate::pool::Pool;
use crate::value::Value;

/// A transaction in progress.
#[derive(Debug)]
pub struct Tx {
    inner: Mutex<Option<sqlx::Transaction<'static, Any>>>,
}

impl Tx {
    /// Begins a new transaction on `pool`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the database cannot begin a transaction.
    pub async fn begin(pool: &Pool) -> Result<Self, Error> {
        let tx = pool.begin().await.map_err(Error::Sqlx)?;
        Ok(Self {
            inner: Mutex::new(Some(tx)),
        })
    }

    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the commit fails.
    pub async fn commit(self) -> Result<(), Error> {
        let tx = self.inner.lock().unwrap().take();
        if let Some(tx) = tx {
            tx.commit().await.map_err(Error::Sqlx)?;
        }
        Ok(())
    }

    /// Rolls back the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] if the rollback fails.
    pub async fn rollback(self) -> Result<(), Error> {
        let tx = self.inner.lock().unwrap().take();
        if let Some(tx) = tx {
            tx.rollback().await.map_err(Error::Sqlx)?;
        }
        Ok(())
    }

    /// Executes a raw statement inside the transaction, returning rows affected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    #[allow(clippy::await_holding_lock)]
    pub async fn execute(&self, sql: &str, binds: Vec<Value>) -> Result<u64, Error> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| Error::Message("transaction mutex poisoned".into()))?;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query::<Any>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.execute(&mut **tx)
            .await
            .map(|r| r.rows_affected())
            .map_err(Error::Sqlx)
    }

    /// Fetches all rows from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    #[allow(clippy::await_holding_lock)]
    pub async fn fetch_all<T>(&self, sql: &str, binds: Vec<Value>) -> Result<Vec<T>, Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| Error::Message("transaction mutex poisoned".into()))?;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query_as::<Any, T>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_all(&mut **tx).await.map_err(Error::Sqlx)
    }

    /// Fetches one row from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    #[allow(clippy::await_holding_lock)]
    pub async fn fetch_one<T>(&self, sql: &str, binds: Vec<Value>) -> Result<T, Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| Error::Message("transaction mutex poisoned".into()))?;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query_as::<Any, T>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_one(&mut **tx).await.map_err(Error::Sqlx)
    }

    /// Fetches one row from a raw statement inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sqlx`] for database errors or [`Error::Message`] if the
    /// transaction has already been finished.
    #[allow(clippy::await_holding_lock)]
    pub async fn fetch_optional<T>(&self, sql: &str, binds: Vec<Value>) -> Result<Option<T>, Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::any::AnyRow>,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| Error::Message("transaction mutex poisoned".into()))?;
        let tx = guard
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))?;

        let mut q = sqlx::query_as::<Any, T>(sql);
        for b in binds {
            q = q.bind(b);
        }
        q.fetch_optional(&mut **tx).await.map_err(Error::Sqlx)
    }
}
