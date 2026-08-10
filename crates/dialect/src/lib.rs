//! SQL dialect abstraction.
//!
//! Implemented in P2; see `ProjectPlan/ImplementationPlan/ImplPlan03DialectsSqlGen.md`.
//!
//! The seam exists from P0 so that no other crate can accidentally grow a
//! hard-coded assumption about Postgres syntax before the abstraction lands.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// Placeholder for the P2 `DbDialect` trait.
///
/// Kept so downstream crates can already name the module path they will depend
/// on, and so the workspace graph is complete from day one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented;
