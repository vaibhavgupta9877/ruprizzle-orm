//! Turso / libSQL adapter for the `ruprizzle` ORM.
//!
//! [`TursoPool`] implements [`ruprizzle::Executor`] by sending each statement to a
//! libSQL server over the [Hrana 2 HTTP protocol][hrana] — `POST {url}/v2/pipeline`
//! with the SQL and its bound parameters, and one result set back. It works against
//! Turso's hosted databases and against any `sqld` you run yourself.
//!
//! [hrana]: https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_3_SPEC.md
//!
//! # What this adapter does and does not do
//!
//! Every statement is one HTTP request. There is no server-side session, so there is
//! no interactive transaction: `BEGIN` and `COMMIT` sent as separate statements would
//! land on unrelated connections and are not supported. Batch the work into a single
//! statement, or use a `SQLite` file through [`ruprizzle::connect`] when you need
//! multi-statement transactions.
//!
//! Embedded replicas — a local `SQLite` file kept in sync with a remote primary — need
//! the native libSQL library and are out of scope here. If you already have a replica
//! file, point [`ruprizzle::connect`] at it as an ordinary `SQLite` database.
//!
//! # Example
//!
//! ```no_run
//! use ruprizzle::Executor;
//! use ruprizzle::value::Value;
//! use ruprizzle_turso::TursoPool;
//!
//! # async fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let pool = TursoPool::builder()
//!     .url("libsql://your-db.turso.io")
//!     .auth_token(std::env::var("TURSO_AUTH_TOKEN")?)
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

mod hrana;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use futures_core::future::BoxFuture;
use futures_util::StreamExt as _;
use ruprizzle::executor::{BoxRowStream, Executor, RawRow, RowBatch};
use ruprizzle::value::Value;
use ruprizzle_dialect::{DbDialect, SqliteDialect};
use thiserror::Error;

use hrana::{Pipeline, PipelineResponse, Stmt};

/// Dialect instance for Turso / libSQL, which is `SQLite` compatible.
static SQLITE_DIALECT: SqliteDialect = SqliteDialect;

/// How long to wait for a single statement before giving up.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors raised by the Turso adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TursoError {
    /// The pool was configured with something it cannot use.
    #[error("turso configuration error: {0}")]
    Config(String),

    /// The request never completed: DNS, TLS, connection or timeout.
    #[error("turso transport error: {0}")]
    Transport(String),

    /// The server answered, but not with a `/v2/pipeline` response we can read.
    #[error("turso protocol error: {0}")]
    Protocol(String),

    /// The server rejected the statement.
    #[error("turso query error: {message}{}", .code.as_deref().map(|c| format!(" ({c})")).unwrap_or_default())]
    Query {
        /// The message the server sent.
        message: String,
        /// The `SQLITE_*` code, when the server sent one.
        code: Option<String>,
    },

    /// The operation has no representation in `SQLite` or in stateless HTTP.
    #[error("unsupported by the Turso HTTP adapter: {0}")]
    Unsupported(String),
}

impl From<TursoError> for ruprizzle::Error {
    fn from(err: TursoError) -> Self {
        ruprizzle::Error::Message(err.to_string())
    }
}

/// Builder for [`TursoPool`].
#[derive(Debug, Default)]
pub struct TursoPoolBuilder {
    url: Option<String>,
    auth_token: Option<String>,
    timeout: Option<Duration>,
}

impl TursoPoolBuilder {
    /// Creates a builder with no configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the database URL.
    ///
    /// `libsql://…` and `wss://…` are accepted and rewritten to `https://`; an
    /// explicit `http://` or `https://` is used as given.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the bearer token sent with every request.
    ///
    /// Omit it only for a local `sqld` that runs without authentication.
    #[must_use]
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
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
    /// This does not contact the server; the first statement does. Nothing here can
    /// tell you the token is valid, so failures surface where the query is issued.
    ///
    /// # Errors
    ///
    /// Returns [`TursoError::Config`] if no URL was set or its scheme is not one of
    /// `libsql`, `ws`, `wss`, `http` or `https`, and if the HTTP client cannot be
    /// constructed (which on most platforms means no usable TLS root store).
    pub fn build(self) -> Result<TursoPool, TursoError> {
        let url = self
            .url
            .ok_or_else(|| TursoError::Config("no database URL was configured".into()))?;
        let endpoint = pipeline_endpoint(&url)?;

        let http = reqwest::Client::builder()
            .timeout(self.timeout.unwrap_or(DEFAULT_TIMEOUT))
            .build()
            .map_err(|e| TursoError::Config(format!("cannot build the HTTP client: {e}")))?;

        Ok(TursoPool {
            inner: Arc::new(TursoPoolInner {
                http,
                endpoint,
                database_url: url,
                auth_token: self.auth_token,
            }),
        })
    }
}

/// Rewrites a database URL into the `/v2/pipeline` endpoint to post to.
///
/// `libsql://` and `wss://` are the schemes Turso publishes; both speak HTTPS to the
/// same host. `ws://` and `http://` are for a local `sqld` without TLS.
fn pipeline_endpoint(url: &str) -> Result<String, TursoError> {
    let (scheme, rest) = url.split_once("://").ok_or_else(|| {
        TursoError::Config(format!("`{url}` has no scheme; expected `libsql://…`"))
    })?;
    let base = match scheme {
        "libsql" | "wss" | "https" => "https",
        "ws" | "http" => "http",
        other => {
            return Err(TursoError::Config(format!(
                "unsupported scheme `{other}`; expected libsql, wss, ws, https or http"
            )));
        }
    };
    if rest.is_empty() {
        return Err(TursoError::Config(format!("`{url}` names no host")));
    }
    Ok(format!(
        "{base}://{}/v2/pipeline",
        rest.trim_end_matches('/')
    ))
}

struct TursoPoolInner {
    http: reqwest::Client,
    /// The full `…/v2/pipeline` URL every request is posted to.
    endpoint: String,
    /// The URL as configured, for diagnostics. Never contains the token.
    database_url: String,
    auth_token: Option<String>,
}

/// Redacts the auth token.
///
/// `TursoPool` is held inside application state that is routinely logged, and a
/// derived `Debug` would put a database credential in the log line.
impl std::fmt::Debug for TursoPoolInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TursoPoolInner")
            .field("endpoint", &self.endpoint)
            .field("database_url", &self.database_url)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            // `http` is omitted: a `reqwest::Client` renders as an opaque blob that
            // tells a reader nothing the endpoint has not already said.
            .finish_non_exhaustive()
    }
}

/// A handle to a libSQL database over Hrana HTTP.
///
/// Cloning is cheap and shares one connection pool, so clone this rather than
/// building a second one.
#[derive(Debug, Clone)]
pub struct TursoPool {
    inner: Arc<TursoPoolInner>,
}

impl TursoPool {
    /// Returns a new [`TursoPoolBuilder`].
    #[must_use]
    pub fn builder() -> TursoPoolBuilder {
        TursoPoolBuilder::new()
    }

    /// Connects using a URL and token, the common case.
    ///
    /// # Errors
    ///
    /// As [`TursoPoolBuilder::build`].
    pub fn connect(url: &str, auth_token: impl Into<String>) -> Result<Self, TursoError> {
        Self::builder().url(url).auth_token(auth_token).build()
    }

    /// The database URL this pool was configured with.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.inner.database_url
    }

    /// Sends one statement and returns the server's result.
    async fn pipeline(
        &self,
        sql: &str,
        binds: &[Value],
        want_rows: bool,
    ) -> Result<hrana::ExecuteResult, TursoError> {
        let args = binds
            .iter()
            .map(hrana::to_hrana)
            .collect::<Result<Vec<_>, _>>()?;

        let body = Pipeline::one(Stmt {
            sql: sql.to_owned(),
            args,
            want_rows,
        });

        let mut request = self.inner.http.post(&self.inner.endpoint).json(&body);
        if let Some(token) = &self.inner.auth_token {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|e| TursoError::Transport(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| TursoError::Transport(format!("cannot read the response body: {e}")))?;

        if !status.is_success() {
            // The body is the server's explanation and is worth keeping, but it is
            // untrusted and unbounded, so it is truncated rather than logged whole.
            return Err(TursoError::Protocol(format!(
                "server answered {status}: {}",
                truncate(&text, 512)
            )));
        }

        let parsed: PipelineResponse = serde_json::from_str(&text).map_err(|e| {
            TursoError::Protocol(format!(
                "cannot parse the pipeline response: {e}; body was {}",
                truncate(&text, 512)
            ))
        })?;

        hrana::single_execute(parsed)
    }
}

/// Shortens `text` to at most `limit` characters, on a character boundary.
fn truncate(text: &str, limit: usize) -> Cow<'_, str> {
    match text.char_indices().nth(limit) {
        Some((end, _)) => Cow::Owned(format!("{}…", &text[..end])),
        None => Cow::Borrowed(text),
    }
}

impl Executor for TursoPool {
    fn dialect(&self) -> &dyn DbDialect {
        &SQLITE_DIALECT
    }

    fn fetch_all_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<RowBatch, ruprizzle::Error>> {
        Box::pin(async move {
            let result = self.pipeline(&sql, &binds, true).await?;
            Ok(RowBatch::Edge(result.into_edge_rows()?))
        })
    }

    fn execute_raw(
        &self,
        sql: Cow<'static, str>,
        binds: Vec<Value>,
    ) -> BoxFuture<'_, Result<u64, ruprizzle::Error>> {
        Box::pin(async move {
            // `want_rows: false` so an `INSERT … RETURNING` does not ship a result
            // set across the network that the caller has said it will not read.
            let result = self.pipeline(&sql, &binds, false).await?;
            Ok(result.affected_row_count)
        })
    }

    fn stream_raw(&self, sql: Cow<'static, str>, binds: Vec<Value>) -> BoxRowStream<'_> {
        // Hrana over HTTP returns the whole result set in one response, so this
        // fetches and then yields. It is a streaming interface, not a streaming
        // transport: memory use is that of the full result.
        Box::pin(
            futures_util::stream::once(self.fetch_all_raw(sql, binds)).flat_map(|batch| {
                let rows = match batch {
                    Ok(RowBatch::Edge(rows)) => {
                        rows.into_iter().map(RawRow::Edge).map(Ok).collect()
                    }
                    Ok(_) => vec![Err(ruprizzle::Error::Message(
                        "the Turso adapter only produces edge rows".into(),
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

    #[test]
    fn turso_urls_are_rewritten_to_the_pipeline_endpoint() {
        assert_eq!(
            pipeline_endpoint("libsql://db.turso.io").unwrap(),
            "https://db.turso.io/v2/pipeline"
        );
        assert_eq!(
            pipeline_endpoint("wss://db.turso.io/").unwrap(),
            "https://db.turso.io/v2/pipeline"
        );
        assert_eq!(
            pipeline_endpoint("http://127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080/v2/pipeline"
        );
    }

    #[test]
    fn an_unusable_url_is_refused_at_build_time() {
        assert!(matches!(
            pipeline_endpoint("db.turso.io"),
            Err(TursoError::Config(_))
        ));
        assert!(matches!(
            pipeline_endpoint("postgres://db.turso.io"),
            Err(TursoError::Config(_))
        ));
        assert!(matches!(
            TursoPool::builder().auth_token("t").build(),
            Err(TursoError::Config(_))
        ));
    }

    #[test]
    fn the_configured_url_is_reported_without_the_token() {
        let pool = TursoPool::connect("libsql://db.turso.io", "secret-token").unwrap();
        assert_eq!(pool.url(), "libsql://db.turso.io");
        let rendered = format!("{pool:?}");
        assert!(
            !rendered.contains("secret-token"),
            "the auth token must not be in Debug output: {rendered}"
        );
    }

    #[test]
    fn the_dialect_is_sqlite() {
        let pool = TursoPool::connect("libsql://db.turso.io", "t").unwrap();
        assert_eq!(pool.dialect().name(), "sqlite");
    }
}
