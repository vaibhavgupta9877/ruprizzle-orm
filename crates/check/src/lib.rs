//! Offline query checking for ruprizzle.
//!
//! Validates captured queries and `raw!` fragments against a schema file
//! without requiring a live database.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

pub mod manifest;
pub mod report;
pub mod validate;

pub use manifest::{ColumnSpec, ParamSpec, QueryEntry, QueryManifest, SourceLocation};
pub use report::{ReportFormat, format_report};
pub use validate::{QueryCheckError, validate_manifest, validate_query_entry, validate_raw};
