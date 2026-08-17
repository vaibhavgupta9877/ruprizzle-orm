//! IR to Rust source emission.
//!
//! Generates typed entities, column constants, query builders, and `FromRow`
//! implementations from the `Schema` produced by `ruprizzle_parser`. Emission
//! goes through `quote!` and `prettyplease` rather than string formatting, so
//! generated output cannot be syntactically invalid Rust.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod emit;

pub use emit::generate_all;
