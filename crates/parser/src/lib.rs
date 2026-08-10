//! `schema.ruprizzle` parser.
//!
//! Implemented in P1; see `ProjectPlan/ImplementationPlan/ImplPlan02SchemaDslParser.md`.
//!
//! The public surface will be a single function,
//! `parse(source: &str) -> Result<Schema, SchemaErrors>`. Keeping the boundary
//! that narrow is deliberate: it is what makes swapping the parser
//! implementation a contained change if Pest proves awkward.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// Placeholder for the P1 parser entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented;
