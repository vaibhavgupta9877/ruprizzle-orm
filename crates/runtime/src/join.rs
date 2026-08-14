//! Join result types and offset-aware decoding.
//!
//! `Join2<A, B>` and `LeftJoin2<A, B>` let joined queries decode directly into
//! pairs of model types. `A` is decoded from the start of the row and `B` is
//! decoded from an `OffsetRow` view that begins at `A::COLUMNS.len()`.

use std::ops::{Deref, DerefMut};

use sqlx::{
    any::AnyRow, mysql::MySqlRow, postgres::PgRow, sqlite::SqliteRow, ColumnIndex, Row, ValueRef,
};

use crate::col::Column;
use crate::filter::{CmpOp, Filter, FilterNode};
use crate::model::{Model, RowDecode};
use crate::offset_row::OffsetRow;

/// Which side of a join to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// `INNER JOIN`.
    Inner,
    /// `LEFT JOIN`.
    Left,
    /// `RIGHT JOIN`.
    Right,
    /// `FULL JOIN`.
    Full,
}

/// A join condition. Internally it is just a `FilterNode`, so it can be:
/// - a column-column comparison (`User::id.eq_col(Post::user_id)`)
/// - a value predicate on either side (`Post::published.eq(true)`)
/// - combinations with `.and()` / `.or()` / `.not()`
#[derive(Debug, Clone, PartialEq)]
pub struct JoinOn {
    pub(crate) node: FilterNode,
}

impl JoinOn {
    /// Create a join condition that equates two typed columns.
    pub fn new<M, J, T>(left: Column<M, T>, right: Column<J, T>) -> Self {
        Self {
            node: FilterNode::ColumnCmp {
                left_table: left.table,
                left_col: left.column,
                op: CmpOp::Eq,
                right_table: right.table,
                right_col: right.column,
            },
        }
    }

    /// Combine two join conditions with `AND`.
    pub fn and(self, other: impl Into<JoinOn>) -> Self {
        let other = other.into();
        Self {
            node: FilterNode::And(vec![self.node, other.node]),
        }
    }

    /// Combine two join conditions with `OR`.
    pub fn or(self, other: impl Into<JoinOn>) -> Self {
        let other = other.into();
        Self {
            node: FilterNode::Or(vec![self.node, other.node]),
        }
    }
}

impl<M> From<Filter<M>> for JoinOn {
    fn from(f: Filter<M>) -> Self {
        Self { node: f.node }
    }
}

/// Internal description of a join attached to a [`SelectQuery`](crate::query::SelectQuery).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JoinSpec {
    pub kind: JoinKind,
    pub right_table: &'static str,
    pub right_columns: &'static [&'static str],
    pub right_alias: Option<&'static str>,
    pub on: JoinOn,
}

/// A model that can appear as one side of an explicit join.
///
/// A join side is a [`Model`] that can also decode itself from an
/// [`OffsetRow`] view of a concrete `sqlx` row type `R`. This offset-aware
/// method is what lets `Join2` and `LeftJoin2` place `B`’s columns immediately
/// after `A`’s columns without needing table-qualified aliases.
pub trait JoinSide<R: Row>: Model + RowDecode
where
    usize: ColumnIndex<R>,
{
    /// Decode `self` from an `OffsetRow` view of the concrete `sqlx` row `R`.
    ///
    /// `row` is a view into the original result set; its column `0` is the
    /// first column of this model's block, regardless of where that block
    /// appears in the wider row.
    fn from_offset_row<'r>(row: &OffsetRow<'r, R>) -> Result<Self, sqlx::Error>
    where
        Self: Sized;
}

/// Newtype around `Option<B>` for the right-hand side of an outer join.
///
/// This wrapper exists so `ruprizzle` can implement `sqlx::FromRow` and the
/// native row traits for an optional joined row. It converts freely with
/// `Option<B>` and dereferences to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Maybe<B>(pub Option<B>);

impl<B> Maybe<B> {
    /// Returns `true` if the right-hand side was not present.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.0.is_none()
    }

    /// Returns `true` if the right-hand side was present.
    #[must_use]
    pub const fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// Returns a reference to the inner value, if any.
    #[must_use]
    pub const fn as_ref(&self) -> Option<&B> {
        self.0.as_ref()
    }

    /// Unwraps the inner value, panicking if `None`.
    pub fn unwrap(self) -> B {
        self.0.unwrap()
    }
}

impl<B> From<Maybe<B>> for Option<B> {
    fn from(m: Maybe<B>) -> Self {
        m.0
    }
}

impl<B> From<Option<B>> for Maybe<B> {
    fn from(o: Option<B>) -> Self {
        Maybe(o)
    }
}

impl<B> Deref for Maybe<B> {
    type Target = Option<B>;

    fn deref(&self) -> &Option<B> {
        &self.0
    }
}

impl<B> DerefMut for Maybe<B> {
    fn deref_mut(&mut self) -> &mut Option<B> {
        &mut self.0
    }
}

impl<B: Model + RowDecode> Model for Maybe<B> {
    const TABLE: &'static str = B::TABLE;
    const PRIMARY_KEY: &'static str = B::PRIMARY_KEY;
    const COLUMNS: &'static [&'static str] = B::COLUMNS;
}

impl<R: Row, B: JoinSide<R>> JoinSide<R> for Maybe<B>
where
    usize: ColumnIndex<R>,
{
    fn from_offset_row<'r>(row: &OffsetRow<'r, R>) -> Result<Self, sqlx::Error>
    where
        Self: Sized,
    {
        let mut all_null = true;
        for i in 0..B::COLUMNS.len() {
            if !row.try_get_raw(i)?.is_null() {
                all_null = false;
                break;
            }
        }
        if all_null {
            Ok(Maybe(None))
        } else {
            Ok(Maybe(Some(B::from_offset_row(row)?)))
        }
    }
}

/// Result of an inner join: `(A, B)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join2<A, B>(pub A, pub B);

impl<A, B> From<Join2<A, B>> for (A, B) {
    fn from(j: Join2<A, B>) -> Self {
        (j.0, j.1)
    }
}

impl<A, B> From<(A, B)> for Join2<A, B> {
    fn from(t: (A, B)) -> Self {
        Join2(t.0, t.1)
    }
}

/// Result of a left (or right/full) outer join: `(A, Option<B>)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftJoin2<A, B>(pub A, pub Maybe<B>);

impl<A, B> From<LeftJoin2<A, B>> for (A, Option<B>) {
    fn from(j: LeftJoin2<A, B>) -> Self {
        (j.0, j.1.into())
    }
}

impl<A, B> From<(A, Option<B>)> for LeftJoin2<A, B> {
    fn from(t: (A, Option<B>)) -> Self {
        LeftJoin2(t.0, t.1.into())
    }
}

// ----------------------------------------------------------------------------
// sqlx concrete-row decoding
// ----------------------------------------------------------------------------

macro_rules! impl_sqlx_from_row {
    ($row:ty) => {
        impl<'r, A: JoinSide<$row>, B: JoinSide<$row>> sqlx::FromRow<'r, $row> for Join2<A, B> {
            fn from_row(row: &'r $row) -> Result<Self, sqlx::Error> {
                let a = A::from_row(row)?;
                let offset = A::COLUMNS.len();
                let b_row = OffsetRow::new(&*row, offset, B::COLUMNS.len());
                let b = B::from_offset_row(&b_row)?;
                Ok(Join2(a, b))
            }
        }

        impl<'r, A: JoinSide<$row>, B: JoinSide<$row>> sqlx::FromRow<'r, $row> for LeftJoin2<A, B> {
            fn from_row(row: &'r $row) -> Result<Self, sqlx::Error> {
                let a = A::from_row(row)?;
                let offset = A::COLUMNS.len();
                let mut all_null = true;
                for i in 0..B::COLUMNS.len() {
                    if !row.try_get_raw(offset + i)?.is_null() {
                        all_null = false;
                        break;
                    }
                }
                let b = if all_null {
                    Maybe(None)
                } else {
                    let b_row = OffsetRow::new(&*row, offset, B::COLUMNS.len());
                    Maybe(Some(B::from_offset_row(&b_row)?))
                };
                Ok(LeftJoin2(a, b))
            }
        }

        impl<'r, B: Model + RowDecode> sqlx::FromRow<'r, $row> for Maybe<B> {
            fn from_row(row: &'r $row) -> Result<Self, sqlx::Error> {
                let mut all_null = true;
                for i in 0..B::COLUMNS.len() {
                    if !row.try_get_raw(i)?.is_null() {
                        all_null = false;
                        break;
                    }
                }
                if all_null {
                    Ok(Maybe(None))
                } else {
                    Ok(Maybe(Some(B::from_row(row)?)))
                }
            }
        }
    };
}

impl_sqlx_from_row!(AnyRow);
impl_sqlx_from_row!(PgRow);
impl_sqlx_from_row!(SqliteRow);
impl_sqlx_from_row!(MySqlRow);

// ----------------------------------------------------------------------------
// rusqlite row decoding
// ----------------------------------------------------------------------------

#[cfg(feature = "sqlite-rusqlite")]
mod rusqlite_impl {
    use super::*;
    use crate::Error;
    use crate::rusqlite::{FromOwnedRow, FromRusqliteRow, Row, RusqliteRow, RusqliteValue};

    impl<A: Model + RowDecode, B: Model + RowDecode> FromOwnedRow for Join2<A, B> {
        fn from_owned_row(row: &Row) -> Result<Self, Error> {
            let a_len = A::COLUMNS.len();
            let a_row = Row {
                values: row.values[0..a_len].to_vec(),
                names: row.names[0..a_len].to_vec(),
            };
            let a = A::from_owned_row(&a_row)?;
            let b_len = B::COLUMNS.len();
            let b_row = Row {
                values: row.values[a_len..a_len + b_len].to_vec(),
                names: row.names[a_len..a_len + b_len].to_vec(),
            };
            let b = B::from_owned_row(&b_row)?;
            Ok(Join2(a, b))
        }
    }

    impl<A: Model + RowDecode, B: Model + RowDecode> FromOwnedRow for LeftJoin2<A, B> {
        fn from_owned_row(row: &Row) -> Result<Self, Error> {
            let a_len = A::COLUMNS.len();
            let a_row = Row {
                values: row.values[0..a_len].to_vec(),
                names: row.names[0..a_len].to_vec(),
            };
            let a = A::from_owned_row(&a_row)?;
            let b_len = B::COLUMNS.len();
            let b_row = Row {
                values: row.values[a_len..a_len + b_len].to_vec(),
                names: row.names[a_len..a_len + b_len].to_vec(),
            };
            let b = Maybe::<B>::from_owned_row(&b_row)?;
            Ok(LeftJoin2(a, b))
        }
    }

    impl<B: Model + RowDecode> FromOwnedRow for Maybe<B> {
        fn from_owned_row(row: &Row) -> Result<Self, Error> {
            if row.values.iter().all(|v| matches!(v, RusqliteValue::Null)) {
                Ok(Maybe(None))
            } else {
                Ok(Maybe(Some(B::from_owned_row(row)?)))
            }
        }
    }

    impl<A: Model + RowDecode, B: Model + RowDecode> FromRusqliteRow for Join2<A, B> {
        fn from_rusqlite_row(row: &RusqliteRow) -> Result<Self, Error> {
            let owned = rusqlite_row_to_owned(row)?;
            Self::from_owned_row(&owned)
        }
    }

    impl<A: Model + RowDecode, B: Model + RowDecode> FromRusqliteRow for LeftJoin2<A, B> {
        fn from_rusqlite_row(row: &RusqliteRow) -> Result<Self, Error> {
            let owned = rusqlite_row_to_owned(row)?;
            Self::from_owned_row(&owned)
        }
    }

    impl<B: Model + RowDecode> FromRusqliteRow for Maybe<B> {
        fn from_rusqlite_row(row: &RusqliteRow) -> Result<Self, Error> {
            let owned = rusqlite_row_to_owned(row)?;
            Self::from_owned_row(&owned)
        }
    }

    pub(crate) fn rusqlite_row_to_owned(row: &RusqliteRow) -> Result<Row, Error> {
        let stmt = row.as_ref();
        let names = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut values = Vec::with_capacity(names.len());
        for i in 0..names.len() {
            let v = row
                .get::<_, RusqliteValue>(i)
                .map_err(|e| Error::Message(e.to_string()))?;
            values.push(v);
        }
        Ok(Row { values, names })
    }
}

// ----------------------------------------------------------------------------
// tokio-postgres row decoding
// ----------------------------------------------------------------------------

#[cfg(feature = "postgres-tokio-postgres")]
mod tokio_postgres_impl {
    use super::*;
    use crate::Error;
    use crate::tokio_postgres::{FromTokioPostgresRow, Row};

    impl<A: RowDecode, B: RowDecode> FromTokioPostgresRow for Join2<A, B> {
        fn from_tokio_postgres_row(_row: &Row) -> Result<Self, Error> {
            Err(Error::Message("tokio-postgres joins not yet implemented".into()))
        }
    }

    impl<A: RowDecode, B: RowDecode> FromTokioPostgresRow for LeftJoin2<A, B> {
        fn from_tokio_postgres_row(_row: &Row) -> Result<Self, Error> {
            Err(Error::Message("tokio-postgres joins not yet implemented".into()))
        }
    }

    impl<B: RowDecode> FromTokioPostgresRow for Maybe<B> {
        fn from_tokio_postgres_row(_row: &Row) -> Result<Self, Error> {
            Err(Error::Message("tokio-postgres joins not yet implemented".into()))
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use sqlx::Row;
    use sqlx_core::any::{AnyColumn, AnyRow, AnyTypeInfo, AnyTypeInfoKind, AnyValue, AnyValueKind};

    use super::*;
    use crate::model::Model;
    use crate::offset_row::OffsetRow;

    #[derive(Debug, Default, PartialEq)]
    struct User {
        id: i64,
        name: String,
    }

    #[derive(Debug, Default, PartialEq)]
    struct Post {
        id: i64,
        title: String,
    }

    impl Model for User {
        const TABLE: &'static str = "users";
        const COLUMNS: &'static [&'static str] = &["id", "name"];
    }

    impl Model for Post {
        const TABLE: &'static str = "posts";
        const COLUMNS: &'static [&'static str] = &["id", "title"];
    }

    impl<'r, R: Row> sqlx::FromRow<'r, R> for User
    where
        usize: sqlx::ColumnIndex<R>,
        i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
        String: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    {
        fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
            Ok(User {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
            })
        }
    }

    impl<R: Row> JoinSide<R> for User
    where
        usize: sqlx::ColumnIndex<R>,
        i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
        String: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    {
        fn from_offset_row<'r>(row: &OffsetRow<'r, R>) -> Result<Self, sqlx::Error> {
            Ok(User {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
            })
        }
    }

    impl<'r, R: Row> sqlx::FromRow<'r, R> for Post
    where
        usize: sqlx::ColumnIndex<R>,
        i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
        String: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    {
        fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
            Ok(Post {
                id: row.try_get(0)?,
                title: row.try_get(1)?,
            })
        }
    }

    impl<R: Row> JoinSide<R> for Post
    where
        usize: sqlx::ColumnIndex<R>,
        i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
        String: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    {
        fn from_offset_row<'r>(row: &OffsetRow<'r, R>) -> Result<Self, sqlx::Error> {
            Ok(Post {
                id: row.try_get(0)?,
                title: row.try_get(1)?,
            })
        }
    }

    #[cfg(feature = "sqlite-rusqlite")]
    mod rusqlite_test_models {
        use super::*;
        use crate::Error;
        use crate::rusqlite::{FromOwnedRow, FromRusqliteRow, Row, RusqliteValue};

        impl FromOwnedRow for User {
            fn from_owned_row(row: &Row) -> Result<Self, Error> {
                let id = match &row.values[0] {
                    RusqliteValue::Integer(i) => *i,
                    _ => 0,
                };
                let name = match &row.values[1] {
                    RusqliteValue::Text(s) => s.clone(),
                    _ => String::new(),
                };
                Ok(User { id, name })
            }
        }

        impl FromOwnedRow for Post {
            fn from_owned_row(row: &Row) -> Result<Self, Error> {
                let id = match &row.values[0] {
                    RusqliteValue::Integer(i) => *i,
                    _ => 0,
                };
                let title = match &row.values[1] {
                    RusqliteValue::Text(s) => s.clone(),
                    _ => String::new(),
                };
                Ok(Post { id, title })
            }
        }

        impl FromRusqliteRow for User {
            fn from_rusqlite_row(row: &crate::rusqlite::RusqliteRow) -> Result<Self, Error> {
                let owned = crate::join::rusqlite_impl::rusqlite_row_to_owned(row)?;
                Self::from_owned_row(&owned)
            }
        }

        impl FromRusqliteRow for Post {
            fn from_rusqlite_row(row: &crate::rusqlite::RusqliteRow) -> Result<Self, Error> {
                let owned = crate::join::rusqlite_impl::rusqlite_row_to_owned(row)?;
                Self::from_owned_row(&owned)
            }
        }
    }

    #[cfg(feature = "postgres-tokio-postgres")]
    mod tokio_postgres_test_models {
        use super::*;
        use crate::Error;
        use crate::tokio_postgres::FromTokioPostgresRow;

        impl FromTokioPostgresRow for User {
            fn from_tokio_postgres_row(_: &crate::tokio_postgres::Row) -> Result<Self, Error> {
                Ok(User::default())
            }
        }

        impl FromTokioPostgresRow for Post {
            fn from_tokio_postgres_row(_: &crate::tokio_postgres::Row) -> Result<Self, Error> {
                Ok(Post::default())
            }
        }
    }

    fn any_column(ordinal: usize, name: &'static str, kind: AnyTypeInfoKind) -> AnyColumn {
        AnyColumn {
            ordinal,
            name: name.to_string().into(),
            type_info: AnyTypeInfo { kind },
        }
    }

    fn any_row(
        user_id: i64,
        user_name: &'static str,
        post_id: Option<i64>,
        post_title: Option<&'static str>,
    ) -> AnyRow {
        let values = vec![
            AnyValue {
                kind: AnyValueKind::BigInt(user_id),
            },
            AnyValue {
                kind: AnyValueKind::Text(Cow::Borrowed(user_name)),
            },
            AnyValue {
                kind: post_id
                    .map(AnyValueKind::BigInt)
                    .unwrap_or(AnyValueKind::Null(AnyTypeInfoKind::BigInt)),
            },
            AnyValue {
                kind: post_title
                    .map(|s| AnyValueKind::Text(Cow::Borrowed(s)))
                    .unwrap_or(AnyValueKind::Null(AnyTypeInfoKind::Text)),
            },
        ];
        let columns = vec![
            any_column(0, "id", AnyTypeInfoKind::BigInt),
            any_column(1, "name", AnyTypeInfoKind::Text),
            any_column(2, "post_id", AnyTypeInfoKind::BigInt),
            any_column(3, "title", AnyTypeInfoKind::Text),
        ];
        AnyRow {
            column_names: Arc::new(Default::default()),
            columns,
            values,
        }
    }

    #[test]
    fn join2_decodes_any_row() {
        let row = any_row(1, "alice", Some(10), Some("hello"));
        let join: Join2<User, Post> = sqlx::FromRow::from_row(&row).unwrap();
        assert_eq!(
            join.0,
            User {
                id: 1,
                name: "alice".into()
            }
        );
        assert_eq!(
            join.1,
            Post {
                id: 10,
                title: "hello".into()
            }
        );
    }

    #[test]
    fn left_join2_decodes_any_row() {
        let row = any_row(1, "alice", Some(10), Some("hello"));
        let left: LeftJoin2<User, Post> = sqlx::FromRow::from_row(&row).unwrap();
        assert_eq!(
            left.0,
            User {
                id: 1,
                name: "alice".into()
            }
        );
        assert!(left.1.is_some());
        assert_eq!(
            left.1.as_ref().unwrap(),
            &Post {
                id: 10,
                title: "hello".into()
            }
        );
    }

    #[test]
    fn left_join2_decodes_null_side() {
        let row = any_row(1, "alice", None, None);
        let left: LeftJoin2<User, Post> = sqlx::FromRow::from_row(&row).unwrap();
        assert_eq!(
            left.0,
            User {
                id: 1,
                name: "alice".into()
            }
        );
        assert!(left.1.is_none());
    }

    #[test]
    fn maybe_decodes_any_row() {
        let values = vec![
            AnyValue {
                kind: AnyValueKind::BigInt(10),
            },
            AnyValue {
                kind: AnyValueKind::Text(Cow::Borrowed("hello")),
            },
        ];
        let columns = vec![
            any_column(0, "id", AnyTypeInfoKind::BigInt),
            any_column(1, "title", AnyTypeInfoKind::Text),
        ];
        let row = AnyRow {
            column_names: Arc::new(Default::default()),
            columns,
            values,
        };
        let maybe: Maybe<Post> = sqlx::FromRow::from_row(&row).unwrap();
        assert!(maybe.is_some());
        assert_eq!(
            maybe.unwrap(),
            Post {
                id: 10,
                title: "hello".into()
            }
        );
    }

    #[test]
    fn maybe_decodes_null_block() {
        let values = vec![
            AnyValue {
                kind: AnyValueKind::Null(AnyTypeInfoKind::BigInt),
            },
            AnyValue {
                kind: AnyValueKind::Null(AnyTypeInfoKind::Text),
            },
        ];
        let columns = vec![
            any_column(0, "id", AnyTypeInfoKind::BigInt),
            any_column(1, "title", AnyTypeInfoKind::Text),
        ];
        let row = AnyRow {
            column_names: Arc::new(Default::default()),
            columns,
            values,
        };
        let maybe: Maybe<Post> = sqlx::FromRow::from_row(&row).unwrap();
        assert!(maybe.is_none());
    }
}
