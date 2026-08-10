//! The `Model` trait that generated entities implement.

/// A generated entity type.
pub trait Model: Sized + Send + Sync + 'static {
    /// The table this model maps to.
    const TABLE: &'static str;
}
