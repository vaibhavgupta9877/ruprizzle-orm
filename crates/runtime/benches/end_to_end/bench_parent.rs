#![allow(clippy::all, clippy::pedantic, unused_imports)]
use ::ruprizzle::serde::{Deserialize, Serialize};
use ::ruprizzle::sqlx::any::AnyRow;
use ::ruprizzle::types::chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use ::ruprizzle::types::{Decimal, Uuid};
use ::ruprizzle::serde_json::Value as JsonValue;
use ::ruprizzle::Related;
use ::ruprizzle::Column;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchParent {
    pub id: i64,
    pub name: String,
    #[serde(skip_serializing_if = "::ruprizzle::Related::is_absent", default)]
    pub children: ::ruprizzle::Related<Vec<super::bench_child::BenchChild>>,
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, AnyRow> for BenchParent {
    fn from_row(row: &'r AnyRow) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            children: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::postgres::PgRow>
for BenchParent {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::postgres::PgRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            children: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::sqlite::SqliteRow>
for BenchParent {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::sqlite::SqliteRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            children: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromRusqliteRow for BenchParent {
    fn from_rusqlite_row(
        row: &::ruprizzle::rusqlite::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::Row::get::<i64>(&row, 0)?,
            name: ::ruprizzle::rusqlite::Row::get::<String>(&row, 1)?,
            children: ::ruprizzle::Related::default(),
        })
    }
}
/// Insert shape: required fields are required, defaulted/optional fields
/// are `Option` so the database can fill them in.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchParentInsert {
    pub id: i64,
    pub name: String,
}
/// Update shape: every field is `Option` to support explicit nulls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchParentUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl ::ruprizzle::Model for BenchParent {
    const TABLE: &'static str = "bench_parents";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name"];
}
/// Table name for this model.
pub const TABLE: &str = "bench_parents";
pub const ID: Column<BenchParent, i64> = Column::new("bench_parents", "id");
pub const NAME: Column<BenchParent, String> = Column::new("bench_parents", "name");
/// Returns an `IncludeList` for this relation.
pub fn children() -> ::ruprizzle::IncludeList<
    'static,
    BenchParent,
    super::bench_child::BenchChild,
    i64,
    (),
> {
    ::ruprizzle::IncludeList::new(
        |__row| __row.id,
        |__row, __loaded| __row.children = __loaded,
        super::bench_child::PARENT_ID,
        |__child| __child.parent_id,
    )
}
/// Returns a filter for parents that have at least one matching child.
pub fn children_some(
    f: ::ruprizzle::Filter<super::bench_child::BenchChild>,
) -> ::ruprizzle::Filter<BenchParent> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_children",
        child_col: "parent_id",
        parent_table: "bench_parents",
        parent_col: "id",
        filter: Box::new(f.node),
        negated: false,
    })
}
/// Returns a filter for parents that have no matching child.
pub fn children_none(
    f: ::ruprizzle::Filter<super::bench_child::BenchChild>,
) -> ::ruprizzle::Filter<BenchParent> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_children",
        child_col: "parent_id",
        parent_table: "bench_parents",
        parent_col: "id",
        filter: Box::new(f.node),
        negated: true,
    })
}
/// Returns a filter for parents where every matching child satisfies `f`.
/// Vacuously true for parents with no children.
pub fn children_every(
    f: ::ruprizzle::Filter<super::bench_child::BenchChild>,
) -> ::ruprizzle::Filter<BenchParent> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_children",
        child_col: "parent_id",
        parent_table: "bench_parents",
        parent_col: "id",
        filter: Box::new((!f).node),
        negated: true,
    })
}
/// Prisma-flavoured repository for `#model_name`.
#[derive(Debug, Clone, Copy)]
pub struct BenchParentRepo<'a> {
    db: &'a super::Db,
}
impl<'a> BenchParentRepo<'a> {
    /// Creates a new repository handle.
    pub(crate) fn new(db: &'a super::Db) -> Self {
        Self { db }
    }
    /// Start a `find_many` query.
    pub fn find_many(&self) -> ::ruprizzle::SelectQuery<'a, BenchParent> {
        ::ruprizzle::SelectQuery::new(self.db.raw_pool())
    }
    /// Start an `insert` query.
    pub fn create(
        &self,
        _data: BenchParentInsert,
    ) -> ::ruprizzle::InsertQuery<'a, BenchParent> {
        let mut insert = ::ruprizzle::InsertQuery::new(self.db.raw_pool());
        insert = insert.set(ID, _data.id);
        insert = insert.set(NAME, _data.name);
        insert
    }
    /// Start a multi-row `insert` query.
    pub fn create_many(
        &self,
        _data: Vec<BenchParentInsert>,
    ) -> ::ruprizzle::InsertManyQuery<'a, BenchParent> {
        let mut q = ::ruprizzle::InsertManyQuery::new(self.db.raw_pool());
        for _row in _data {
            q = q
                .row([
                    ("id", ::ruprizzle::Encodable::to_value(&_row.id)),
                    ("name", ::ruprizzle::Encodable::to_value(&_row.name)),
                ]);
        }
        q
    }
    /// Start an `update` query.
    pub fn update(&self) -> ::ruprizzle::UpdateQuery<'a, BenchParent> {
        ::ruprizzle::UpdateQuery::new(self.db.raw_pool())
    }
    /// Start a `delete` query.
    pub fn delete(&self) -> ::ruprizzle::DeleteQuery<'a, BenchParent> {
        ::ruprizzle::DeleteQuery::new(self.db.raw_pool())
    }
}
