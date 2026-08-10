//! The ruprizzle runtime: the crate your application depends on.
//!
//! This crate provides the types that generated code compiles against:
//! typed columns, filters, relation wrappers, and the query builders that the
//! P4 implementation will fill in.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod col;
pub mod compile;
pub mod error;
pub mod filter;
pub mod model;
pub mod order;
pub mod pool;
pub mod query;
pub mod related;
pub mod tx;
pub mod value;

/// Re-exported database types (chrono, uuid, decimal, json, ...) so generated
/// code can depend on `ruprizzle` alone.
pub mod types {
    pub use sqlx::types::*;
}

/// Common imports for application code.
pub mod prelude {
    pub use crate::{
        Column, Encodable, Error, Filter, InsertQuery, Model, OrderBy, Pool, Related, SelectQuery,
        Tx, Value,
    };
}

pub use col::{Column, Projection};
pub use compile::{CompiledSql, delete, dialect_for_pool, insert, select, update};
pub use error::Error;
pub use filter::{Filter, FilterNode, all, any};
pub use model::Model;
pub use order::OrderBy;
pub use pool::{Pool, connect};
pub use query::{DeleteQuery, InsertQuery, SelectQuery, UpdateQuery};
pub use related::Related;
pub use serde;
pub use serde_json;
pub use sqlx;
pub use tx::Tx;
pub use value::{Encodable, Ordered, Value};

/// A boxed future, used by the transaction escape hatch.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
