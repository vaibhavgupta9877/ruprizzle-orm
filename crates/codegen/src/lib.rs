//! IR to Rust source emission.
//!
//! Implemented in P3; see `ProjectPlan/ImplementationPlan/ImplPlan04CodegenEntities.md`.
//!
//! Emission goes through `quote!` and `prettyplease` rather than string
//! formatting, so generated output cannot be syntactically invalid Rust.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// Placeholder for the P3 emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented;
