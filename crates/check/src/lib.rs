//! Offline query checking for ruprizzle.
//!
//! Validates captured queries and `raw!` fragments against a schema file
//! without requiring a live database.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

pub mod manifest;
pub mod validate;

pub use manifest::{QueryEntry, QueryManifest};
pub use validate::{QueryCheckError, validate_manifest, validate_raw};
