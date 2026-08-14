//! Typed aggregate expressions.
//!
//! Aggregates are built from [`Column`] tokens and carry the return type at the
//! type level, so `User::age.sum()` has type `Aggregate<User, Option<i64>>` when
//! `age` is an `Int` and `User::id.count()` has type `Aggregate<User, i64>`.

use std::marker::PhantomData;

use crate::col::Column;
use crate::model::RowDecode;

#[cfg(feature = "postgres-tokio-postgres")]
use tokio_postgres::types::FromSqlOwned;

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
/// numeric. Implementations are provided for the integer and float types
/// ruprizzle supports, plus `Option<T>` for nullable numeric columns.
///
/// `Decimal` is intentionally omitted from the first pass because `sqlx::Any`
/// and SQLite do not provide native `sqlx::Decode`/`sqlx::Type` for
/// `rust_decimal::Decimal`; aggregate decoding would require a custom
/// `FromRow` path that is left for a follow-up.
pub trait Numeric: Send + Sync + 'static {
    /// The Rust type returned by `SUM` over this column.
    ///
    /// Widens `i32` to `i64` and keeps `f64` unchanged so the
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

/// Trait for scalar types that can be decoded from a single aggregate result
/// column.
///
/// This is the output-side counterpart of `Numeric`: the result set produced by
/// `SUM`, `AVG`, etc. contains one value per aggregate, and that value must be
/// decodable by every active backend driver.

#[cfg(all(feature = "sqlite-rusqlite", not(feature = "postgres-tokio-postgres")))]
pub trait AggregateScalar:
    Send + Sync + 'static
    + for<'r> sqlx::Decode<'r, sqlx::Any>
    + sqlx::Type<sqlx::Any>
    + for<'r> sqlx::Decode<'r, sqlx::Postgres>
    + sqlx::Type<sqlx::Postgres>
    + for<'r> sqlx::Decode<'r, sqlx::Sqlite>
    + sqlx::Type<sqlx::Sqlite>
    + for<'r> sqlx::Decode<'r, sqlx::MySql>
    + sqlx::Type<sqlx::MySql>
    + crate::rusqlite::FromValue
{
}

/// Scalar types that can be decoded from an aggregate result column when the
/// native `tokio-postgres` backend is enabled.
#[cfg(all(not(feature = "sqlite-rusqlite"), feature = "postgres-tokio-postgres"))]
pub trait AggregateScalar:
    Send + Sync + 'static
    + for<'r> sqlx::Decode<'r, sqlx::Any>
    + sqlx::Type<sqlx::Any>
    + for<'r> sqlx::Decode<'r, sqlx::Postgres>
    + sqlx::Type<sqlx::Postgres>
    + for<'r> sqlx::Decode<'r, sqlx::Sqlite>
    + sqlx::Type<sqlx::Sqlite>
    + for<'r> sqlx::Decode<'r, sqlx::MySql>
    + sqlx::Type<sqlx::MySql>
    + FromSqlOwned
{
}

/// Scalar types that can be decoded from an aggregate result column when both
/// native backends are enabled.
#[cfg(all(feature = "sqlite-rusqlite", feature = "postgres-tokio-postgres"))]
pub trait AggregateScalar:
    Send + Sync + 'static
    + for<'r> sqlx::Decode<'r, sqlx::Any>
    + sqlx::Type<sqlx::Any>
    + for<'r> sqlx::Decode<'r, sqlx::Postgres>
    + sqlx::Type<sqlx::Postgres>
    + for<'r> sqlx::Decode<'r, sqlx::Sqlite>
    + sqlx::Type<sqlx::Sqlite>
    + for<'r> sqlx::Decode<'r, sqlx::MySql>
    + sqlx::Type<sqlx::MySql>
    + crate::rusqlite::FromValue
    + FromSqlOwned
{
}

/// Scalar types that can be decoded from an aggregate result column when no
/// native backend is enabled.
#[cfg(all(not(feature = "sqlite-rusqlite"), not(feature = "postgres-tokio-postgres")))]
pub trait AggregateScalar:
    Send + Sync + 'static
    + for<'r> sqlx::Decode<'r, sqlx::Any>
    + sqlx::Type<sqlx::Any>
    + for<'r> sqlx::Decode<'r, sqlx::Postgres>
    + sqlx::Type<sqlx::Postgres>
    + for<'r> sqlx::Decode<'r, sqlx::Sqlite>
    + sqlx::Type<sqlx::Sqlite>
    + for<'r> sqlx::Decode<'r, sqlx::MySql>
    + sqlx::Type<sqlx::MySql>
{
}

impl AggregateScalar for i32 {}
impl AggregateScalar for i64 {}
impl AggregateScalar for f64 {}
impl<T: AggregateScalar> AggregateScalar for Option<T> {}

/// A single aggregate item in the SQL projection.
#[derive(Debug, Clone)]
pub struct AggregateEntry {
    /// The table the aggregate is over.
    pub table: &'static str,
    /// The column the aggregate is over.
    pub column: &'static str,
    /// The kind of aggregate.
    pub kind: AggregateKind,
    /// The alias the aggregate uses in the result set.
    pub alias: String,
}

/// Trait for a single aggregate expression that maps to a scalar output.
pub trait IntoAggregate<M> {
    /// The scalar Rust type the single aggregate decodes into.
    type Out: AggregateScalar;

    /// Pushes the aggregate entry into `out`.
    fn push_entry(&self, out: &mut Vec<AggregateEntry>);
}

impl<M, T: AggregateScalar> IntoAggregate<M> for Aggregate<M, T> {
    type Out = T;

    fn push_entry(&self, out: &mut Vec<AggregateEntry>) {
        out.push(AggregateEntry {
            table: self.table,
            column: self.column,
            kind: self.kind,
            alias: self.alias(),
        });
    }
}

/// Trait for a set of aggregate expressions that defines an aggregate query.
///
/// Implementations are provided for a single aggregate and for tuples of up to
/// eight aggregates, matching the arity generated applications most often need.
pub trait AggregateSet<M, R: RowDecode> {
    /// Pushes each aggregate entry into `out`.
    fn push_entries(&self, out: &mut Vec<AggregateEntry>);
}

impl<M, T: AggregateScalar> AggregateSet<M, (T,)> for Aggregate<M, T> {
    fn push_entries(&self, out: &mut Vec<AggregateEntry>) {
        IntoAggregate::push_entry(self, out);
    }
}

macro_rules! impl_aggregate_set_tuples {
    ($($n:tt $T:ident),+) => {
        impl<M, $($T),+> AggregateSet<M, ($($T::Out,)+)> for ($($T,)+)
        where
            $( $T: IntoAggregate<M>, )+
        {
            fn push_entries(&self, out: &mut Vec<AggregateEntry>) {
                $(
                    self.$n.push_entry(out);
                )+
            }
        }
    };
}

impl_aggregate_set_tuples! { 0 A0 }
impl_aggregate_set_tuples! { 0 A0, 1 A1 }
impl_aggregate_set_tuples! { 0 A0, 1 A1, 2 A2 }
impl_aggregate_set_tuples! { 0 A0, 1 A1, 2 A2, 3 A3 }
impl_aggregate_set_tuples! { 0 A0, 1 A1, 2 A2, 3 A3, 4 A4 }
impl_aggregate_set_tuples! { 0 A0, 1 A1, 2 A2, 3 A3, 4 A4, 5 A5 }
impl_aggregate_set_tuples! { 0 A0, 1 A1, 2 A2, 3 A3, 4 A4, 5 A5, 6 A6 }
impl_aggregate_set_tuples! { 0 A0, 1 A1, 2 A2, 3 A3, 4 A4, 5 A5, 6 A6, 7 A7 }

/// Trait for column sets that can appear in `GROUP BY`.
pub trait GroupBy<M> {
    /// The column names, in order.
    fn columns(&self) -> Vec<&'static str>;
}

impl<M, T> GroupBy<M> for Column<M, T> {
    fn columns(&self) -> Vec<&'static str> {
        vec![self.column]
    }
}

macro_rules! impl_group_by_tuples {
    ($($n:tt $T:ident),+) => {
        impl<M, $($T),+> GroupBy<M> for ($($T,)+)
        where
            $( $T: GroupBy<M>, )+
        {
            fn columns(&self) -> Vec<&'static str> {
                let mut cols = Vec::new();
                $(
                    cols.extend(self.$n.columns());
                )+
                cols
            }
        }
    };
}

impl_group_by_tuples! { 0 G0 }
impl_group_by_tuples! { 0 G0, 1 G1 }
impl_group_by_tuples! { 0 G0, 1 G1, 2 G2 }
impl_group_by_tuples! { 0 G0, 1 G1, 2 G2, 3 G3 }
impl_group_by_tuples! { 0 G0, 1 G1, 2 G2, 3 G3, 4 G4 }
impl_group_by_tuples! { 0 G0, 1 G1, 2 G2, 3 G3, 4 G4, 5 G5 }
impl_group_by_tuples! { 0 G0, 1 G1, 2 G2, 3 G3, 4 G4, 5 G5, 6 G6 }
impl_group_by_tuples! { 0 G0, 1 G1, 2 G2, 3 G3, 4 G4, 5 G5, 6 G6, 7 G7 }
