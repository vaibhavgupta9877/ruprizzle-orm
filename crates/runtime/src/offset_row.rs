//! Shifted view into a concrete `sqlx` row.
//!
//! `OffsetRow` is not an `sqlx::Row` itself: the `sqlx::Row` trait carries a
//! `'static` bound and a `Database::Row = Self` requirement that a lifetime-
//! carrying wrapper cannot satisfy. Instead, `OffsetRow` exposes the same
//! `try_get` and `try_get_raw` surface and forwards calls to the underlying
//! row at `index + offset`.

use std::fmt;

use sqlx::{Column, ColumnIndex, Database, Decode, Row, Type};

/// A view into a concrete `sqlx` row where column `0` is the column at
/// `offset` of the original row.
pub struct OffsetRow<'r, R: ?Sized> {
    row: &'r R,
    offset: usize,
    len: usize,
}

impl<'r, R: ?Sized> OffsetRow<'r, R> {
    /// Create a new offset view of `row` starting at `offset` and spanning `len`
    /// columns.
    pub const fn new(row: &'r R, offset: usize, len: usize) -> Self {
        OffsetRow { row, offset, len }
    }

    /// The number of columns visible in this view.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the view contains no columns.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The offset into the underlying row where this view begins.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// The underlying row that this view indexes into.
    pub const fn as_raw(&self) -> &'r R {
        self.row
    }
}

impl<'r, R: ?Sized> fmt::Debug for OffsetRow<'r, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OffsetRow")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .finish()
    }
}

impl<'r, R: Row + ?Sized> OffsetRow<'r, R>
where
    usize: ColumnIndex<R>,
{
    /// Decode a single value from the view by index or name.
    pub fn try_get<'a, T, I>(&'a self, index: I) -> Result<T, sqlx::Error>
    where
        I: ColumnIndex<OffsetRow<'r, R>>,
        T: Decode<'a, R::Database> + Type<R::Database>,
    {
        let idx = index.index(self)?;
        self.row.try_get(self.offset + idx)
    }

    /// Access the raw value at `index` in the view without decoding it.
    pub fn try_get_raw<'a, I>(
        &'a self,
        index: I,
    ) -> Result<<R::Database as Database>::ValueRef<'a>, sqlx::Error>
    where
        I: ColumnIndex<OffsetRow<'r, R>>,
    {
        let idx = index.index(self)?;
        self.row.try_get_raw(self.offset + idx)
    }
}

impl<'r, R: Row + ?Sized> ColumnIndex<OffsetRow<'r, R>> for usize {
    fn index(&self, offset_row: &OffsetRow<'r, R>) -> Result<usize, sqlx::Error> {
        if *self >= offset_row.len {
            return Err(sqlx::Error::ColumnIndexOutOfBounds {
                index: *self,
                len: offset_row.len,
            });
        }
        Ok(*self)
    }
}

impl<'r, R: Row + ?Sized> ColumnIndex<OffsetRow<'r, R>> for &str {
    fn index(&self, offset_row: &OffsetRow<'r, R>) -> Result<usize, sqlx::Error> {
        for col in offset_row.row.columns() {
            if col.name() == *self {
                let idx = col.ordinal();
                if idx < offset_row.offset || idx >= offset_row.offset + offset_row.len {
                    return Err(sqlx::Error::ColumnNotFound(self.to_string()));
                }
                return Ok(idx - offset_row.offset);
            }
        }
        Err(sqlx::Error::ColumnNotFound(self.to_string()))
    }
}
