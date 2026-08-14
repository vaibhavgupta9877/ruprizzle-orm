//! Snapshot, diff, plan, apply.
//!
//! Computes the difference between the target `Schema` and the schema stored in
//! the database, emits up/down SQL, and applies migrations in order under
//! advisory locking. The snapshot format is the serialized
//! `ruprizzle_core::ir::Schema`, so this crate never defines a second
//! description of a schema.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(
    clippy::assigning_clones,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::default_trait_access,
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::uninlined_format_args,
    clippy::unnecessary_map_or,
    clippy::unused_self
)]

pub mod change;
pub mod diff;
pub mod drift;
pub mod error;
pub mod introspect;
pub mod plan;
pub mod runner;

pub use change::{Change, ColumnAspect};
pub use diff::diff;
pub use drift::detect;
pub use error::Error;
pub use plan::{down_sql, plan, up_sql};
pub use runner::{Migration, MigrationMeta, Migrator, Report, Status};
