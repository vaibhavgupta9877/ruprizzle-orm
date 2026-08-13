//! Relation wrapper that distinguishes "not loaded" from "loaded and empty".

use serde::{Deserialize, Serialize};

/// A relation field that may or may not have been loaded.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Related<T> {
    /// Not requested in this query. Reading it is a programmer error.
    #[default]
    Absent,
    /// Loaded value.
    Loaded(T),
}

impl<T> Related<T> {
    /// Returns `true` if the relation was not loaded.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Returns `true` if the relation was loaded.
    #[must_use]
    pub const fn is_loaded(&self) -> bool {
        !self.is_absent()
    }

    /// Returns a reference to the loaded value.
    ///
    /// # Panics
    ///
    /// Panics if `self` is `Absent`, with a message naming the missing relation.
    #[must_use]
    pub fn get(&self) -> &T {
        match self {
            Self::Loaded(v) => v,
            Self::Absent => panic!("relation was not loaded — add an `.include()` and execute the query with `.exec()` or `.exec_one()`"),
        }
    }

    /// Returns the loaded value, if any.
    #[must_use]
    pub const fn try_get(&self) -> Option<&T> {
        match self {
            Self::Loaded(v) => Some(v),
            Self::Absent => None,
        }
    }
}
