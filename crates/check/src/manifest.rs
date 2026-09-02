//! Manifest of captured queries for offline validation.

use serde::{Deserialize, Serialize};

/// Source code location where a query was defined or captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// File path.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    #[serde(default)]
    pub column: u32,
}

/// Specification of an expected bind parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Name of the parameter if named.
    #[serde(default)]
    pub name: Option<String>,
    /// 1-based position or index.
    pub position: usize,
    /// Expected scalar type name (e.g. `Int`, `String`, `Boolean`).
    pub expected_type: String,
    /// Whether null values are allowed.
    #[serde(default)]
    pub nullable: bool,
}

/// Specification of an expected result column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSpec {
    /// Column name in the projection.
    pub name: String,
    /// Inferred or declared type name.
    pub inferred_type: String,
    /// Whether the column is nullable.
    #[serde(default)]
    pub nullable: bool,
}

/// A captured query and its semantic metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryEntry {
    /// Optional identifier for the query.
    #[serde(default)]
    pub id: Option<String>,
    /// SQL with placeholders or raw SQL statement.
    pub sql: String,
    /// Dialect used to compile or validate the query (`postgres`, `sqlite`, `mysql`).
    pub dialect: String,
    /// Expected bind parameter specifications.
    #[serde(default)]
    pub params: Vec<ParamSpec>,
    /// Expected result column specifications.
    #[serde(default)]
    pub result_columns: Vec<ColumnSpec>,
    /// Source file, if recorded.
    #[serde(default)]
    pub source: Option<String>,
    /// Source line, if recorded.
    #[serde(default)]
    pub line: Option<u32>,
    /// Source code location details.
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

/// A set of queries captured against a schema snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryManifest {
    /// Manifest format version.
    #[serde(default = "default_version")]
    pub version: u32,
    /// SHA-256 hash / fingerprint of the schema that the queries were compiled against.
    #[serde(alias = "schema_fingerprint")]
    pub schema_hash: String,
    /// Captured queries, in the order they were recorded.
    pub queries: Vec<QueryEntry>,
}

fn default_version() -> u32 {
    1
}

impl QueryManifest {
    /// Creates a new empty `QueryManifest` for a given schema hash.
    #[must_use]
    pub fn new(schema_hash: impl Into<String>) -> Self {
        Self {
            version: 1,
            schema_hash: schema_hash.into(),
            queries: Vec::new(),
        }
    }

    /// Verifies if the manifest schema fingerprint matches the given schema.
    #[must_use]
    pub fn matches_schema(&self, schema: &ruprizzle_core::ir::Schema) -> bool {
        if self.schema_hash.is_empty() {
            return true;
        }
        let fp = schema.fingerprint();
        self.schema_hash == fp || self.schema_hash.starts_with(&fp[..fp.len().min(8)])
    }
}
