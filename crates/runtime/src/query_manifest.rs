//! Optional query-manifest recorder for offline validation.
//!
//! When `RUPRIZZLE_RECORD_QUERIES` is set, every public `to_sql()` call appends
//! an entry to an in-memory buffer. The buffer is written to disk when
//! `write_manifest` is called.

use std::sync::Mutex;
use std::{env, fs, io, path::Path, sync::OnceLock};

/// One captured query.
#[derive(Debug, Clone)]
pub struct QueryEntry {
    /// SQL with placeholders.
    pub sql: String,
    /// Source file, if recorded.
    pub source: Option<&'static str>,
    /// Source line, if recorded.
    pub line: Option<u32>,
    /// Dialect used to compile the query.
    pub dialect: String,
}

static RECORDING: OnceLock<Mutex<Vec<QueryEntry>>> = OnceLock::new();

/// Record a query if `RUPRIZZLE_RECORD_QUERIES` is set.
pub fn record(sql: String, source: Option<&'static str>, line: Option<u32>, dialect: &str) {
    if env::var("RUPRIZZLE_RECORD_QUERIES").is_ok() {
        if let Ok(mut guard) = RECORDING.get_or_init(Mutex::default).lock() {
            guard.push(QueryEntry {
                sql,
                source,
                line,
                dialect: dialect.to_owned(),
            });
        }
    }
}

/// Clear the in-memory recording buffer.
///
/// Intended for tests; not normally called by application code.
#[doc(hidden)]
pub fn clear() {
    if let Some(m) = RECORDING.get() {
        if let Ok(mut guard) = m.lock() {
            guard.clear();
        }
    }
}

/// Write all recorded queries to `path` as JSON.
pub fn write_manifest<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let entries = RECORDING
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.clone())
        .unwrap_or_default();
    let manifest = ruprizzle_check::QueryManifest {
        version: 1,
        schema_hash: String::new(), // schema hash is computed by the caller of write_manifest
        queries: entries
            .into_iter()
            .map(|e| ruprizzle_check::QueryEntry {
                id: None,
                sql: e.sql,
                source: e.source.map(String::from),
                line: e.line,
                dialect: e.dialect,
                params: Vec::new(),
                result_columns: Vec::new(),
                location: None,
            })
            .collect(),
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(path, json)
}
