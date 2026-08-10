//! Order-by tokens.

use std::marker::PhantomData;

/// A typed order-by clause.
#[derive(Debug, PartialEq, Eq)]
pub struct OrderBy<M> {
    /// The SQL table name.
    pub table: &'static str,
    /// The SQL column name.
    pub column: &'static str,
    /// `true` for `DESC`, `false` for `ASC`.
    pub desc: bool,
    _marker: PhantomData<fn() -> M>,
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
