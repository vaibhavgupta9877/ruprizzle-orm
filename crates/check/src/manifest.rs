//! Manifest of captured queries for offline validation.

use serde::{Deserialize, Serialize};

/// A captured query and its source location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryEntry {
    /// SQL with placeholders.
    pub sql: String,
    /// Source file, if recorded.
    pub source: Option<String>,
    /// Source line, if recorded.
    pub line: Option<u32>,
    /// Dialect used to compile the query.
    pub dialect: String,
}

/// A set of queries captured against a schema snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryManifest {
    /// SHA-256 hash of the schema that the queries were compiled against.
    pub schema_hash: String,
    /// Captured queries, in the order they were recorded.
    pub queries: Vec<QueryEntry>,
}
