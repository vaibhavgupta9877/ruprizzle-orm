//! Proc-macro support for the ruprizzle runtime.
//!
//! Implemented alongside P4; see
//! `ProjectPlan/ImplementationPlan/ImplPlan05QueryBuilderRuntime.md`.
//!
//! This crate stays deliberately thin. The ORM's type safety comes from
//! generated column tokens (ADR-005), not from macro magic, so the only macros
//! here are conveniences such as the injection-safe `raw!` fragment builder.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

// Proc-macro crates may only export macros, so the P0 placeholder is a private
// helper rather than a public type.
#[allow(dead_code)]
fn placeholder_until_p4() {}
