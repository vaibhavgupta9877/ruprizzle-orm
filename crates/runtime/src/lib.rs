//! The ruprizzle runtime: the crate your application depends on.
//!
//! Implemented in P4; see
//! `ProjectPlan/ImplementationPlan/ImplPlan05QueryBuilderRuntime.md`.
//!
//! Note what is *absent* from this crate's dependency graph: the parser and the
//! code generator. Those run in the CLI, so your application never compiles
//! them.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// Placeholder for the P4 query builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented;
