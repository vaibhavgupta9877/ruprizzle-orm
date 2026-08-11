//! Pagination results.

/// One page of results.
///
/// `has_next` is computed by fetching one row more than requested and then
/// discarding it, which is why it is exact rather than a guess derived from
/// `items.len() == limit`. That guess is wrong precisely when the last page is
/// exactly full, and the resulting phantom "next page" is a classic pagination
/// bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T, C = crate::value::Value> {
    /// The rows on this page.
    pub items: Vec<T>,
    /// Whether another page exists after this one.
    pub has_next: bool,
    /// The cursor to pass to fetch the next page, if any.
    pub next_cursor: Option<C>,
}

impl<T, C> Page<T, C> {
    /// Creates a page.
    pub const fn new(items: Vec<T>, has_next: bool, next_cursor: Option<C>) -> Self {
        Self {
            items,
            has_next,
            next_cursor,
        }
    }

    /// Number of rows on this page.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether this page has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
