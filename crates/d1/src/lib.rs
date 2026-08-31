//! Cloudflare D1 adapter for the `ruprizzle` ORM.
//!
//! [`D1Pool`] implements [`ruprizzle::Executor`] against a D1 database through the
//! [Cloudflare REST API][api]: each statement is one
//! `POST /accounts/{account}/d1/database/{database}/query`, authenticated with an
//! API token.
//!
//! [api]: https://developers.cloudflare.com/api/resources/d1/
//!
//! # What this adapter does and does not do
//!
//! This is the adapter for code that talks to D1 **from outside** a Worker — a CLI, a
//! migration job, a server. Inside a Worker you have a D1 binding, which is faster
//! and needs no token; this crate does not use bindings.
//!
//! Every statement is one HTTP request, so there is no interactive transaction:
//! `BEGIN` and `COMMIT` sent separately would not share a connection and are not
//! supported. D1's HTTP interface also takes no binary parameter, so [`ruprizzle`]
//! `Bytes` values are refused rather than mangled — store them encoded in a text
//! column.
//!
//! # Example
//!
//! ```no_run
//! use ruprizzle::Executor;
//! use ruprizzle::value::Value;
//! use ruprizzle_d1::D1Pool;
//!
//! # async fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let pool = D1Pool::builder()
//!     .account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?)
//!     .database_id(std::env::var("D1_DATABASE_ID")?)
//!     .api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
//!     .build()?;
//!
//! let rows = pool
//!     .fetch_all_raw("SELECT id, email FROM users WHERE id = ?".into(), vec![Value::I64(7)])
//!     .await?;
//! # let _ = rows;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

mod api;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use futures_core::future::BoxFuture;
use futures_util::StreamExt as _;
use ruprizzle::executor::{BoxRowStream, Executor, RawRow, RowBatch};
use ruprizzle::value::Value;
use ruprizzle_dialect::{DbDialect, SqliteDialect};
use thiserror::Error;

use api::{Envelope, QueryRequest};

/// Dialect instance for D1, which is `SQLite`.
static SQLITE_DIALECT: SqliteDialect = SqliteDialect;

/// The Cloudflare API root, overridable for testing and for private gateways.
const DEFAULT_ENDPOINT: &str = "https://api.cloudflare.com/client/v4";

/// How long to wait for a single statement before giving up.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors raised by the D1 adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum D1Error {
    /// The pool was configured with something it cannot use.
    #[error("d1 configuration error: {0}")]
    Config(String),

    /// The request never completed: DNS, TLS, connection or timeout.
    #[error("d1 transport error: {0}")]
    Transport(String),

    /// Cloudflare answered, but not with an envelope we can read.
    #[error("d1 protocol error: {0}")]
    Protocol(String),

    /// Cloudflare reported the statement as failed.
    #[error("d1 query error: {0}")]
    Query(String),

    /// The operation has no representation in D1's HTTP interface.
    #[error("unsupported by the D1 HTTP adapter: {0}")]
    Unsupported(String),
}

impl From<D1Error> for ruprizzle::Error {
    fn from(err: D1Error) -> Self {
        ruprizzle::Error::Message(err.to_string())
    }
}

/// Builder for [`D1Pool`].
#[derive(Debug, Default)]
pub struct D1PoolBuilder {
    account_id: Option<String>,
    database_id: Option<String>,
    api_token: Option<String>,
    endpoint: Option<String>,
    timeout: Option<Duration>,
}

impl D1PoolBuilder {
    /// Creates a builder with no configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Cloudflare account that owns the database.
    #[must_use]
    pub fn account_id(mut self, id: impl Into<String>) -> Self {
        self.account_id = Some(id.into());
        self
    }

    /// Sets the D1 database UUID.
    #[must_use]
    pub fn database_id(mut self, id: impl Into<String>) -> Self {
        self.database_id = Some(id.into());
        self
    }

    /// Sets the API token, sent as a bearer credential on every request.
    ///
    /// The token needs the `D1:edit` permission on the account.
    #[must_use]
    pub fn api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    /// Overrides the API root. Defaults to `https://api.cloudflare.com/client/v4`.
    #[must_use]
    pub fn endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = Some(url.into());
        self
    }

    /// Overrides the per-statement timeout. Defaults to 30 seconds.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Builds the pool.
    ///
    /// This does not contact Cloudflare; the first statement does. Nothing here can
    /// tell you the token is valid or the database exists.
    ///
    /// # Errors
    ///
    /// Returns [`D1Error::Config`] when the account, database or token is missing,
    /// and when the HTTP client cannot be constructed (which on most platforms means
    /// no usable TLS root store).
    pub fn build(self) -> Result<D1Pool, D1Error> {
        let account_id = self
            .account_id
            .ok_or_else(|| D1Error::Config("no `account_id` was configured".into()))?;
        let database_id = self
            .database_id
            .ok_or_else(|| D1Error::Config("no `database_id` was configured".into()))?;
        let api_token = self.api_token.ok_or_else(|| {
            D1Error::Config("no `api_token` was configured; the D1 REST API is not open".into())
        })?;

        let root = self.endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned());
        let query_url = format!(
            "{}/accounts/{account_id}/d1/database/{database_id}/query",
            root.trim_end_matches('/')
        );

        let http = reqwest::Client::builder()
            .timeout(self.timeout.unwrap_or(DEFAULT_TIMEOUT))
            .build()
            .map_err(|e| D1Error::Config(format!("cannot build the HTTP client: {e}")))?;

        Ok(D1Pool {
            inner: Arc::new(D1PoolInner {
                http,
                query_url,
                account_id,
                database_id,
                api_token,
            }),
        })
    }
}

struct D1PoolInner {
    http: reqwest::Client,
    /// The full `…/query` URL every statement is posted to.
    query_url: String,
    account_id: String,
    database_id: String,
    api_token: String,
}

/// Redacts the API token.
///
/// `D1Pool` is held inside application state that is routinely logged, and a derived
/// `Debug` would put a Cloudflare credential in the log line.
impl std::fmt::Debug for D1PoolInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("D1PoolInner")
            .field("query_url", &self.query_url)
            .field("account_id", &self.account_id)
            .field("database_id", &self.database_id)
            .field("api_token", &"<redacted>")
            // `http` is omitted: a `reqwest::Client` renders as an opaque blob that
            // tells a reader nothing the URL has not already said.
            .finish_non_exhaustive()
    }
}

/// A handle to a Cloudflare D1 database over the REST API.
///
/// Cloning is cheap and shares one connection pool, so clone this rather than
/// building a second one.
#[derive(Debug, Clone)]
pub struct D1Pool {
    inner: Arc<D1PoolInner>,
}

impl D1Pool {
    /// Returns a new [`D1PoolBuilder`].
    #[must_use]
    pub fn builder() -> D1PoolBuilder {
        D1PoolBuilder::new()
    }

    /// The Cloudflare account this pool queries.
    #[must_use]
    pub fn account_id(&self) -> &str {
        &self.inner.account_id
    }

    /// The D1 database this pool queries.
    #[must_use]
    pub fn database_id(&self) -> &str {
        &self.inner.database_id
    }

    /// Sends one statement and returns Cloudflare's result for it.
    async fn query(&self, sql: &str, binds: &[Value]) -> Result<api::QueryResult, D1Error> {
        let params = binds
            .iter()
            .map(api::to_json)
            .collect::<Result<Vec<_>, _>>()?;

        let response = self
            .inner
            .http
            .post(&self.inner.query_url)
            .bearer_auth(&self.inner.api_token)
            .json(&QueryRequest {
                sql: sql.to_owned(),
                params,
            })
            .send()
            .await
            .map_err(|e| D1Error::Transport(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| D1Error::Transport(format!("cannot read the response body: {e}")))?;

        // Cloudflare returns its `{success, errors}` envelope on 4xx as well, and
        // that envelope carries the useful message, so it is parsed first and the
        // status is only reported when the body is unreadable.
        match serde_json::from_str::<Envelope>(&text) {
            Ok(envelope) => envelope.single_result(),
            Err(e) if status.is_success() => Err(D1Error::Protocol(format!(
                "cannot parse the D1 response: {e}; body was {}",
                truncate(&text, 512)
            ))),
            Err(_) => Err(D1Error::Protocol(format!(
                "Cloudflare answered {status}: {}",
                truncate(&text, 512)
            ))),
        }
    }
}

/// Shortens `text` to at most `limit` characters, on a character boundary.
fn truncate(text: &str, limit: usize) -> Cow<'_, str> {
    match text.char_indices().nth(limit) {
        Some((end, _)) => Cow::Owned(format!("{}…", &text[..end])),
        None => Cow::Borrowed(text),
    }
}

impl Executor for D1Pool {
    fn dialect(&self) -> &dyn DbDialect {
        &SQLITE_DIALECT
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, ruprizzle::Error>> {
        Box::pin(async move {
            let result = self.query(&sql, &binds).await?;
            Ok(RowBatch::Edge(result.into_edge_rows()))
        })
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, ruprizzle::Error>> {
        Box::pin(async move { Ok(self.query(&sql, &binds).await?.meta.changes) })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        // D1 returns the whole result set in one response, so this fetches and then
        // yields. It is a streaming interface, not a streaming transport: memory use
        // is that of the full result.
        Box::pin(
            futures_util::stream::once(self.fetch_all_raw(sql, binds)).flat_map(|batch| {
                let rows = match batch {
                    Ok(RowBatch::Edge(rows)) => {
                        rows.into_iter().map(RawRow::Edge).map(Ok).collect()
                    }
                    Ok(_) => vec![Err(ruprizzle::Error::Message(
                        "the D1 adapter only produces edge rows".into(),
                    ))],
                    Err(e) => vec![Err(e)],
                };
                futures_util::stream::iter(rows)
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> D1Pool {
        D1Pool::builder()
            .account_id("acct")
            .database_id("db-uuid")
            .api_token("secret-token")
            .build()
            .unwrap()
    }

    #[test]
    fn the_query_url_is_built_from_the_account_and_database() {
        let pool = pool();
        assert_eq!(
            pool.inner.query_url,
            "https://api.cloudflare.com/client/v4/accounts/acct/d1/database/db-uuid/query"
        );
    }

    #[test]
    fn an_overridden_endpoint_keeps_the_path_shape() {
        let pool = D1Pool::builder()
            .account_id("a")
            .database_id("b")
            .api_token("t")
            .endpoint("http://127.0.0.1:9999/")
            .build()
            .unwrap();
        assert_eq!(
            pool.inner.query_url,
            "http://127.0.0.1:9999/accounts/a/d1/database/b/query"
        );
    }

    #[test]
    fn every_credential_is_required() {
        assert!(matches!(
            D1Pool::builder().database_id("b").api_token("t").build(),
            Err(D1Error::Config(_))
        ));
        assert!(matches!(
            D1Pool::builder().account_id("a").api_token("t").build(),
            Err(D1Error::Config(_))
        ));
        assert!(matches!(
            D1Pool::builder().account_id("a").database_id("b").build(),
            Err(D1Error::Config(_))
        ));
    }

    #[test]
    fn the_api_token_is_not_in_debug_output() {
        let rendered = format!("{:?}", pool());
        assert!(
            !rendered.contains("secret-token"),
            "the API token must not be in Debug output: {rendered}"
        );
    }

    #[test]
    fn the_dialect_is_sqlite() {
        assert_eq!(pool().dialect().name(), "sqlite");
    }
}
