//! Owned SQLite migration connections. Session settings outlive transactions,
//! so cleanup must retain the connection through rollback and FK restoration.

use sqlx::{Any, Sqlite};

use crate::{Error, Pool, RowBatch, Value};

#[derive(Debug)]
enum Connection {
    Any(sqlx::pool::PoolConnection<Any>),
    Sqlite(sqlx::pool::PoolConnection<Sqlite>),
}

#[derive(Debug)]
struct CheckedOut {
    connection: Connection,
    clean: bool,
}

impl Drop for CheckedOut {
    fn drop(&mut self) {
        if !self.clean {
            match &mut self.connection {
                Connection::Any(c) => c.close_on_drop(),
                Connection::Sqlite(c) => c.close_on_drop(),
            }
        }
    }
}

impl CheckedOut {
    async fn execute(&mut self, sql: &str, binds: &[Value]) -> Result<u64, Error> {
        match &mut self.connection {
            Connection::Any(c) => {
                let mut q = sqlx::query::<Any>(sql);
                for value in binds {
                    q = q.bind(value);
                }
                q.execute(&mut **c)
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(Error::Sqlx)
            }
            Connection::Sqlite(c) => {
                let mut q = sqlx::query::<Sqlite>(sql);
                for value in binds {
                    q = q.bind(value);
                }
                q.execute(&mut **c)
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(Error::Sqlx)
            }
        }
    }

    async fn fetch(&mut self, sql: &str, binds: &[Value]) -> Result<RowBatch, Error> {
        match &mut self.connection {
            Connection::Any(c) => {
                let mut q = sqlx::query::<Any>(sql);
                for value in binds {
                    q = q.bind(value);
                }
                q.fetch_all(&mut **c)
                    .await
                    .map(RowBatch::Any)
                    .map_err(Error::Sqlx)
            }
            Connection::Sqlite(c) => {
                let mut q = sqlx::query::<Sqlite>(sql);
                for value in binds {
                    q = q.bind(value);
                }
                q.fetch_all(&mut **c)
                    .await
                    .map(RowBatch::Sqlite)
                    .map_err(Error::Sqlx)
            }
        }
    }

    async fn restore(&mut self) -> Result<(), Error> {
        // Also handles a failed BEGIN or a failed COMMIT. A ROLLBACK with no
        // open transaction is harmless; FK restoration must still run.
        let _ = self.execute("ROLLBACK", &[]).await;
        self.execute("PRAGMA foreign_keys = ON", &[]).await?;
        let enabled: i64 = match &mut self.connection {
            Connection::Any(c) => {
                sqlx::query_scalar("PRAGMA foreign_keys")
                    .fetch_one(&mut **c)
                    .await?
            }
            Connection::Sqlite(c) => {
                sqlx::query_scalar("PRAGMA foreign_keys")
                    .fetch_one(&mut **c)
                    .await?
            }
        };
        if enabled != 1 {
            return Err(Error::Message(
                "could not restore SQLite foreign-key enforcement".into(),
            ));
        }
        self.clean = true;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SqliteMigrationConnection {
    conn: Option<CheckedOut>,
}

impl SqliteMigrationConnection {
    pub(crate) async fn begin(pool: &Pool) -> Result<Self, Error> {
        let connection = match pool {
            Pool::Any(p) => Connection::Any(p.acquire().await?),
            Pool::Sqlite(p) => Connection::Sqlite(p.acquire().await?),
            _ => return Err(Error::Message("expected a sqlx SQLite pool".into())),
        };
        let mut this = Self {
            conn: Some(CheckedOut {
                connection,
                clean: false,
            }),
        };
        this.execute("PRAGMA foreign_keys = OFF", &[]).await?;
        this.execute("BEGIN IMMEDIATE", &[]).await?;
        Ok(this)
    }

    fn connection(&mut self) -> Result<&mut CheckedOut, Error> {
        self.conn
            .as_mut()
            .ok_or_else(|| Error::Message("transaction already finished".into()))
    }

    pub(crate) async fn execute(&mut self, sql: &str, binds: &[Value]) -> Result<u64, Error> {
        self.connection()?.execute(sql, binds).await
    }

    pub(crate) async fn fetch(&mut self, sql: &str, binds: &[Value]) -> Result<RowBatch, Error> {
        self.connection()?.fetch(sql, binds).await
    }

    pub(crate) async fn finish(mut self, commit: bool) -> Result<(), Error> {
        let result = self
            .execute(if commit { "COMMIT" } else { "ROLLBACK" }, &[])
            .await;
        let restored = self.connection()?.restore().await;
        // On error/cancellation, CheckedOut closes any connection whose state
        // could not be restored rather than releasing it back to the pool.
        self.conn.take();
        result?;
        restored
    }
}

impl Drop for SqliteMigrationConnection {
    fn drop(&mut self) {
        if let Some(mut conn) = self.conn.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    if let Err(error) = conn.restore().await {
                        tracing::warn!(%error, "failed to restore SQLite migration connection");
                    }
                });
            }
            // Without a runtime CheckedOut closes the dirty connection.
        }
    }
}
