//! Typed aggregate expressions.
//!
//! Aggregates are built from [`Column`] tokens and carry the return type at the
//! type level, so `User::age.sum()` has type `Aggregate<User, Option<i64>>` when
//! `age` is an `Int` and `User::id.count()` has type `Aggregate<User, i64>`.

use std::marker::PhantomData;

use crate::col::Column;
use crate::types::Decimal;

/// A typed aggregate expression for a model `M` with return type `R`.
///
/// `R` is encoded in the phantom data so the compiler can check aggregate
/// result shapes without carrying extra data at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aggregate<M, R> {
    pub(crate) table: &'static str,
    pub(crate) column: &'static str,
    pub(crate) kind: AggregateKind,
    pub(crate) _marker: PhantomData<fn() -> (M, R)>,
}

impl<M, R> Aggregate<M, R> {
    pub(crate) const fn new(table: &'static str, column: &'static str, kind: AggregateKind) -> Self {
        Self {
            table,
            column,
            kind,
            _marker: PhantomData,
        }
    }

    /// Returns the SQL alias this aggregate should use in the projection.
    #[must_use]
    pub fn alias(&self) -> String {
        format!("{}_{}", self.kind.as_str(), self.column)
    }
}

/// Kinds of aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateKind {
    /// `SUM(...)`.
    Sum,
    /// `AVG(...)`.
    Avg,
    /// `MIN(...)`.
    Min,
    /// `MAX(...)`.
    Max,
    /// `COUNT(...)`.
    Count,
    /// `COUNT(DISTINCT ...)`.
    CountDistinct,
}

impl AggregateKind {
    /// The SQL function name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Avg => "avg",
            Self::Min => "min",
            Self::Max => "max",
            Self::Count => "count",
            Self::CountDistinct => "count_distinct",
        }
    }

    /// The SQL function name for emission.
    #[must_use]
    pub const fn sql_fn(&self) -> &'static str {
        match self {
            Self::Sum => "SUM",
            Self::Avg => "AVG",
            Self::Min => "MIN",
            Self::Max => "MAX",
            Self::Count => "COUNT",
            Self::CountDistinct => "COUNT",
        }
    }
}

/// Marker trait for scalar types that can appear in numeric aggregates.
///
/// `Numeric` is sealed so third-party types cannot accidentally claim to be
/// numeric. Implementations are provided for the integer, float, and decimal
/// types ruprizzle supports, plus `Option<T>` for nullable numeric columns.
pub trait Numeric: Send + Sync + 'static {
    /// The Rust type returned by `SUM` over this column.
    ///
    /// Widens `i32` to `i64` and keeps `Decimal` and `f64` unchanged so the
    /// value can be decoded without silent overflow.
    type Sum: Send + Sync + 'static;
    /// The Rust type returned by `AVG` over this column.
    type Avg: Send + Sync + 'static;
    /// The Rust type returned by `MIN` / `MAX` over this column.
    type MinMax: Send + Sync + 'static;
}

impl Numeric for i32 {
    type Sum = i64;
    type Avg = f64;
    type MinMax = i32;
}

impl Numeric for i64 {
    type Sum = i64;
    type Avg = f64;
    type MinMax = i64;
}

impl Numeric for f64 {
    type Sum = f64;
    type Avg = f64;
    type MinMax = f64;
}

impl Numeric for Decimal {
    type Sum = Decimal;
    type Avg = Decimal;
    type MinMax = Decimal;
}

impl<T: Numeric> Numeric for Option<T> {
    type Sum = Option<T::Sum>;
    type Avg = Option<T::Avg>;
    type MinMax = Option<T::MinMax>;
}

impl<M, T: Numeric> Column<M, T> {
    /// `SUM(column)`.
    #[must_use]
    pub fn sum(self) -> Aggregate<M, Option<T::Sum>> {
        Aggregate::new(self.table, self.column, AggregateKind::Sum)
    }

    /// `AVG(column)`.
    #[must_use]
    pub fn avg(self) -> Aggregate<M, Option<T::Avg>> {
        Aggregate::new(self.table, self.column, AggregateKind::Avg)
    }

    /// `MIN(column)`.
    #[must_use]
    pub fn min(self) -> Aggregate<M, Option<T::MinMax>> {
        Aggregate::new(self.table, self.column, AggregateKind::Min)
    }

    /// `MAX(column)`.
    #[must_use]
    pub fn max(self) -> Aggregate<M, Option<T::MinMax>> {
        Aggregate::new(self.table, self.column, AggregateKind::Max)
    }
}

impl<M, T> Column<M, T> {
    /// `COUNT(column)`.
    #[must_use]
    pub fn count(self) -> Aggregate<M, i64> {
        Aggregate::new(self.table, self.column, AggregateKind::Count)
    }

    /// `COUNT(DISTINCT column)`.
    #[must_use]
    pub fn count_distinct(self) -> Aggregate<M, i64> {
        Aggregate::new(self.table, self.column, AggregateKind::CountDistinct)
    }
}
