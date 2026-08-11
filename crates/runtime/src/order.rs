//! Order-by tokens.

use std::fmt;
use std::marker::PhantomData;

/// A typed order-by clause.
#[derive(PartialEq, Eq)]
pub struct OrderBy<M> {
    /// The SQL table name.
    pub table: &'static str,
    /// The SQL column name.
    pub column: &'static str,
    /// `true` for `DESC`, `false` for `ASC`.
    pub desc: bool,
    _marker: PhantomData<fn() -> M>,
}

impl<M> fmt::Debug for OrderBy<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrderBy")
            .field("table", &self.table)
            .field("column", &self.column)
            .field("desc", &self.desc)
            .finish()
    }
}

impl<M> Copy for OrderBy<M> {}
impl<M> Clone for OrderBy<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> OrderBy<M> {
    /// Creates a new order-by token.
    #[must_use]
    pub const fn new(table: &'static str, column: &'static str, desc: bool) -> Self {
        Self {
            table,
            column,
            desc,
            _marker: PhantomData,
        }
    }
}
