//! Source locations.
//!
//! Every IR node carries a [`Span`] so that diagnostics produced anywhere in the
//! pipeline — parsing, validation, codegen, or migration planning — can point at
//! the exact bytes of `schema.ruprizzle` that caused them.
//!
//! Spans are byte offsets, not line/column pairs. Rendering to line/column is the
//! reporter's job (`miette` does it), and byte offsets stay correct under any
//! encoding of the source we choose to keep around.

use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` into the schema source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the first byte of the span.
    pub start: usize,
    /// Byte offset one past the last byte of the span.
    pub end: usize,
}

impl Span {
    /// A span pointing nowhere.
    ///
    /// Used for IR that was constructed programmatically (tests, migration
    /// snapshots read back from disk) rather than parsed from source.
    pub const EMPTY: Span = Span { start: 0, end: 0 };

    /// Creates a span from a half-open byte range.
    ///
    /// If `end` precedes `start` the arguments are swapped, so a malformed span
    /// can never produce a panic downstream in the renderer.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        if end < start {
            Span {
                start: end,
                end: start,
            }
        } else {
            Span { start, end }
        }
    }

    /// Length of the span in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the span covers no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// The smallest span covering both inputs.
    #[must_use]
    pub fn join(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(s: Span) -> Self {
        (s.start, s.len()).into()
    }
}

impl From<std::ops::Range<usize>> for Span {
    fn from(r: std::ops::Range<usize>) -> Self {
        Span::new(r.start, r.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_normalises_reversed_ranges() {
        assert_eq!(Span::new(9, 4), Span { start: 4, end: 9 });
    }

    #[test]
    fn join_covers_both() {
        assert_eq!(Span::new(2, 5).join(Span::new(10, 12)), Span::new(2, 12));
    }

    #[test]
    fn converts_to_miette_span() {
        let s: miette::SourceSpan = Span::new(4, 9).into();
        assert_eq!(s.offset(), 4);
        assert_eq!(s.len(), 5);
    }
}
