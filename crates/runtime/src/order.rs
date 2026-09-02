//! Order-by tokens.

use std::fmt;
use std::marker::PhantomData;

use crate::json::JsonPath;

/// A typed order-by clause.
#[derive(PartialEq, Eq)]
pub struct OrderBy<M> {
    /// The SQL table name.
    pub table: &'static str,
    /// The SQL column name.
    pub column: &'static str,
    /// `true` for `DESC`, `false` for `ASC`.
    pub desc: bool,
    /// Optional JSON path for ordering inside a JSON column.
    pub json_path: Option<JsonPath>,
    /// `true` for text extraction, `false` for JSON.
    pub text: bool,
    /// Optional spatial geometry target for distance ordering (`ST_Distance(col, geom)`).
    pub spatial_distance: Option<String>,
    pub(crate) _marker: PhantomData<fn() -> M>,
}

impl<M> fmt::Debug for OrderBy<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrderBy")
            .field("table", &self.table)
            .field("column", &self.column)
            .field("desc", &self.desc)
            .field("json_path", &self.json_path)
            .field("text", &self.text)
            .field("spatial_distance", &self.spatial_distance)
            .finish()
    }
}

impl<M> Clone for OrderBy<M> {
    fn clone(&self) -> Self {
        Self {
            table: self.table,
            column: self.column,
            desc: self.desc,
            json_path: self.json_path.clone(),
            text: self.text,
            spatial_distance: self.spatial_distance.clone(),
            _marker: PhantomData,
        }
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
            json_path: None,
            text: false,
            spatial_distance: None,
            _marker: PhantomData,
        }
    }

    /// Creates a spatial distance order-by clause (`ST_Distance(col, geom) ASC/DESC`).
    #[must_use]
    pub fn spatial_distance(
        table: &'static str,
        column: &'static str,
        geometry_wkt: String,
        desc: bool,
    ) -> Self {
        Self {
            table,
            column,
            desc,
            json_path: None,
            text: false,
            spatial_distance: Some(geometry_wkt),
            _marker: PhantomData,
        }
    }
}
