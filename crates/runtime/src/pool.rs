//! Connection pool re-exports.

/// A `sqlx` pool over the `Any` driver.
pub type Pool = sqlx::Pool<sqlx::Any>;

/// Connect to a database by URL.
///
/// The URL scheme selects the driver (`postgres://`, `sqlite://`, etc.).
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or the connection fails.
pub async fn connect(url: &str) -> Result<Pool, crate::Error> {
    Pool::connect(url).await.map_err(Into::into)
}
