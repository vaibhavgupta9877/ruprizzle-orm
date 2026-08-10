//! Snapshot, diff, plan, apply.
//!
//! Implemented in P6; see `ProjectPlan/ImplementationPlan/ImplPlan07Migrations.md`.
//!
//! The snapshot format is the serialized [`ruprizzle_core::ir::Schema`] (ADR-007),
//! so this crate never defines a second description of a schema.

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
pub mod error;
pub mod plan;
pub mod runner;

pub use change::{Change, ColumnAspect};
pub use diff::diff;
pub use error::Error;
pub use plan::plan;
pub use runner::{Migration, MigrationMeta, Migrator, Report, Status};
