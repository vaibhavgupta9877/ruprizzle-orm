//! Configuration options for Ruprizzle Studio server.

use std::path::PathBuf;

/// Server configuration flags and options.
#[derive(Debug, Clone)]
pub struct StudioConfig {
    /// Port to listen on (default 5555).
    pub port: u16,
    /// Host address to bind to (default 127.0.0.1).
    pub host: String,
    /// Path to `schema.ruprizzle`.
    pub schema_path: PathBuf,
    /// Database connection URL.
    pub database_url: Option<String>,
    /// Whether mutating write operations (insert, update, delete) are permitted.
    pub allow_writes: bool,
    /// Override guardrail against remote/production database URLs.
    pub yes_i_know: bool,
    /// Disable auto-opening the web browser on launch.
    pub no_browser: bool,
}

impl Default for StudioConfig {
    fn default() -> Self {
        Self {
            port: 5555,
            host: "127.0.0.1".to_string(),
            schema_path: PathBuf::from("schema.ruprizzle"),
            database_url: None,
            allow_writes: false,
            yes_i_know: false,
            no_browser: false,
        }
    }
}
