//! Shared foundations for the ruprizzle ORM.
//!
//! This crate defines the [`ir::Schema`] type — the single contract that the
//! parser produces and that codegen, the dialects, and the migration engine all
//! consume — plus the spans and diagnostics that let any stage of the pipeline
//! point back at the user's source.
//!
//! It has no database dependencies and does no I/O, which keeps it cheap to
//! depend on and trivial to test.
//!
//! # Layout
//!
//! - [`ir`] — the intermediate representation
//! - [`names`] — newtypes distinguishing model, field, and enum names
//! - [`span`] — byte-offset source locations
//! - [`diagnostic`] — accumulating, span-carrying schema errors
//! - [`suggest`] — "did you mean…?" support for those errors

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod diagnostic;
pub mod ir;
pub mod names;
pub mod span;
pub mod suggest;

pub use diagnostic::{Diagnostics, SchemaError, SchemaErrors};
pub use ir::{Provider, Schema};
pub use names::{EnumName, FieldName, ModelName};
pub use span::Span;
