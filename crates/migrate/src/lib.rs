//! Snapshot, diff, plan, apply.
//!
//! Implemented in P6; see `ProjectPlan/ImplementationPlan/ImplPlan07Migrations.md`.
//!
//! The snapshot format is the serialized [`ruprizzle_core::ir::Schema`] (ADR-007),
//! so this crate never defines a second description of a schema.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// Placeholder for the P6 diff engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented;
