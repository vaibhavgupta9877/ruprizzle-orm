//! Newtype wrappers for the different kinds of name in a schema.
//!
//! These exist so that a [`ModelName`] can never be passed where a [`FieldName`]
//! is expected. The IR is threaded through five crates; bare `String` keys would
//! make that plumbing silently mistakable.

use std::borrow::Borrow;
use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! name_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps a raw string.
            #[must_use]
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Borrows the underlying string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Unwraps into the owned string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        // Lets `IndexMap<$name, _>::get("literal")` work without allocating.
        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
    };
}

name_newtype! {
    /// The name of a model as written in the schema, e.g. `User`.
    ///
    /// `PascalCase` by convention; the physical table name lives in
    /// [`Model::table`](crate::ir::Model::table).
    ModelName
}

name_newtype! {
    /// The name of a field within a model, e.g. `createdAt`.
    ///
    /// The physical column name lives in [`Field::column`](crate::ir::Field::column).
    FieldName
}

name_newtype! {
    /// The name of an enum declaration, e.g. `Role`.
    EnumName
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn borrow_allows_str_lookup() {
        let mut m: IndexMap<ModelName, u8> = IndexMap::new();
        m.insert(ModelName::new("User"), 1);
        assert_eq!(m.get("User"), Some(&1));
    }

    #[test]
    fn serialises_transparently() {
        let json = serde_json::to_string(&ModelName::new("User")).unwrap();
        assert_eq!(json, "\"User\"");
    }
}
