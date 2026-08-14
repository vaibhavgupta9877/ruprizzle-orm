//! The ruprizzle runtime: the crate your application depends on.
//!
//! This crate provides the types that generated code compiles against: typed
//! [`Column`]s, [`Filter`]s, [`Related`] wrappers, and the query builders that
//! produce [`CompiledSql`]. It also owns connection pooling ([`Pool`]),
//! transactions ([`Tx`]), query execution ([`Executor`]), and migration helpers
//! re-exported from `ruprizzle_migrate` for the CLI.
//!
//! # Backends
//!
//! By default `ruprizzle` builds on `sqlx::Any`, which lets the same binary talk
//! to Postgres and SQLite. You can opt into native-driver paths:
//!
//! * **`sqlite-rusqlite`** — a synchronous `rusqlite` backend that skips the
//!   `sqlx::Any` text round-trip for SQLite. Enable it in `Cargo.toml` and use a
//!   `driver=rusqlite` query parameter on the SQLite URL.
//! * **`postgres-tokio-postgres`** — experimental native `tokio-postgres`
//!   backend behind the matching feature flag.
//!
//! The public API is identical across backends. Use the typed `Pool::as_any`,
//! `Pool::as_sqlite`, `Pool::as_postgres`, and feature-gated `Pool::as_rusqlite`
//! / `Pool::as_tokio_postgres` accessors when you need driver-specific behaviour.
//!
//! # Prelude
//!
//! Most application code can start with [`prelude`].

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod aggregate;
pub mod col;
pub mod compile;
pub mod counting;
pub mod error;
pub mod executor;
pub mod filter;
pub mod include;
pub mod metrics;
pub mod model;
pub mod order;
pub mod page;
pub mod pool;
pub mod query;
pub mod related;
#[cfg(feature = "sqlite-rusqlite")]
pub mod rusqlite;
#[cfg(feature = "postgres-tokio-postgres")]
pub mod tokio_postgres;
pub mod tx;
pub mod value;

/// Re-exported database types (chrono, uuid, decimal, json, ...) so generated
/// code can depend on `ruprizzle` alone.
pub mod types {
    pub use sqlx::types::*;
}

/// Decoding helpers for generated `FromRow` implementations.
pub mod decode;

/// Common imports for application code.
pub mod prelude {
    pub use crate::{
        Aggregate, Column, Encodable, Error, Executor, Filter, InsertQuery, IsolationLevel, Model,
        Numeric, OrderBy, Page, Pool, RawFragment, Related, SelectQuery, Tx, Value, raw,
    };
}

pub use aggregate::{Aggregate, AggregateKind, Numeric};
pub use col::{Column, Projection};
pub use compile::{CompiledSql, delete, dialect_for_pool, insert, insert_many, select, update};
pub use counting::CountingExecutor;
pub use error::Error;
pub use executor::{Executor, RowBatch, decode_rows};
pub use filter::{Filter, FilterNode, RawFragment, all, any};
pub use include::{IncludeList, IncludeOne, IncludeSet};
pub use model::Model;
pub use order::OrderBy;
pub use page::Page;
pub use pool::{Pool, PoolConfig, PoolStats, connect, connect_with, ping, stats};
pub use query::{
    DeleteQuery, InsertManyQuery, InsertQuery, NestedSetter, SelectQuery, UpdateQuery,
};
pub use related::Related;
pub use ruprizzle_macros::raw;
pub use serde;
pub use serde_json;
pub use sqlx;
pub use tx::{IsolationLevel, Savepoint, Tx, is_retryable};
pub use value::{Encodable, Ordered, Value};

/// A boxed future, used by the transaction escape hatch.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
