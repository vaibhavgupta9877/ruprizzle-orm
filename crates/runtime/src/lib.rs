//! The ruprizzle runtime: the crate your application depends on.
//!
//! This crate provides the types that generated code compiles against:
//! typed columns, filters, relation wrappers, and the query builders that the
//! P4 implementation will fill in.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod col;
pub mod compile;
pub mod counting;
pub mod error;
pub mod executor;
pub mod filter;
pub mod include;
pub mod model;
pub mod order;
pub mod page;
pub mod pool;
pub mod query;
pub mod related;
#[cfg(feature = "sqlite-rusqlite")]
pub mod rusqlite;
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
        Column, Encodable, Error, Executor, Filter, InsertQuery, IsolationLevel, Model, OrderBy,
        Page, Pool, RawFragment, Related, SelectQuery, Tx, Value, raw,
    };
}

pub use col::{Column, Projection};
pub use compile::{CompiledSql, delete, dialect_for_pool, insert, insert_many, select, update};
pub use counting::CountingExecutor;
pub use error::Error;
pub use executor::Executor;
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
pub use tx::{IsolationLevel, Tx, is_retryable};
pub use value::{Encodable, Ordered, Value};

/// A boxed future, used by the transaction escape hatch.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
