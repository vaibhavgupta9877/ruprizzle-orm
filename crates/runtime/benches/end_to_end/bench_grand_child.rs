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
pub struct BenchGrandChild {
    pub id: i64,
    pub child_id: i64,
    pub name: String,
    #[serde(skip_serializing_if = "::ruprizzle::Related::is_absent", default)]
    pub child: ::ruprizzle::Related<Option<super::bench_child::BenchChild>>,
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, AnyRow> for BenchGrandChild {
    fn from_row(row: &'r AnyRow) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            child_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            child: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::postgres::PgRow>
for BenchGrandChild {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::postgres::PgRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            child_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            child: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::sqlite::SqliteRow>
for BenchGrandChild {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::sqlite::SqliteRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            child_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            child: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromRusqliteRow for BenchGrandChild {
    fn from_rusqlite_row(
        row: &::ruprizzle::rusqlite::RusqliteRow,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get_i64(row, 0)?,
            child_id: ::ruprizzle::rusqlite::get_i64(row, 1)?,
            name: ::ruprizzle::rusqlite::get_text(row, 2)?,
            child: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromOwnedRow for BenchGrandChild {
    fn from_owned_row(
        row: &::ruprizzle::rusqlite::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::Row::get::<i64>(row, 0)?,
            child_id: ::ruprizzle::rusqlite::Row::get::<i64>(row, 1)?,
            name: ::ruprizzle::rusqlite::Row::get::<String>(row, 2)?,
            child: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "postgres-tokio-postgres")]
impl ::ruprizzle::tokio_postgres::FromTokioPostgresRow for BenchGrandChild {
    fn from_tokio_postgres_row(
        row: &::ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: row.try_get::<usize, i64>(0).map_err(::ruprizzle::Error::TokioPostgres)?,
            child_id: row
                .try_get::<usize, i64>(1)
                .map_err(::ruprizzle::Error::TokioPostgres)?,
            name: row
                .try_get::<usize, String>(2)
                .map_err(::ruprizzle::Error::TokioPostgres)?,
            child: ::ruprizzle::Related::default(),
        })
    }
}
/// Insert shape: required fields are required, defaulted/optional fields
/// are `Option` so the database can fill them in.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchGrandChildInsert {
    pub id: i64,
    pub child_id: i64,
    pub name: String,
}
/// Update shape: every field is `Option` to support explicit nulls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchGrandChildUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl ::ruprizzle::Model for BenchGrandChild {
    const TABLE: &'static str = "bench_grandchildren";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "child_id", "name"];
}
/// Table name for this model.
pub const TABLE: &str = "bench_grandchildren";
pub const ID: Column<BenchGrandChild, i64> = Column::new("bench_grandchildren", "id");
pub const CHILD_ID: Column<BenchGrandChild, i64> = Column::new(
    "bench_grandchildren",
    "child_id",
);
pub const NAME: Column<BenchGrandChild, String> = Column::new(
    "bench_grandchildren",
    "name",
);
/// Returns an `IncludeOne` for this relation.
pub fn child() -> ::ruprizzle::IncludeOne<
    'static,
    BenchGrandChild,
    super::bench_child::BenchChild,
    i64,
    (),
> {
    ::ruprizzle::IncludeOne::new(
        |__row| __row.child_id,
        |__row, __loaded| __row.child = __loaded,
        super::bench_child::ID,
        |__child| __child.id,
    )
}
/// Returns a filter for parents that have at least one matching child.
pub fn child_some(
    f: ::ruprizzle::Filter<super::bench_child::BenchChild>,
) -> ::ruprizzle::Filter<BenchGrandChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_children",
        child_col: "id",
        parent_table: "bench_grandchildren",
        parent_col: "child_id",
        filter: Box::new(f.node),
        negated: false,
    })
}
/// Returns a filter for parents that have no matching child.
pub fn child_none(
    f: ::ruprizzle::Filter<super::bench_child::BenchChild>,
) -> ::ruprizzle::Filter<BenchGrandChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_children",
        child_col: "id",
        parent_table: "bench_grandchildren",
        parent_col: "child_id",
        filter: Box::new(f.node),
        negated: true,
    })
}
/// Returns a filter for parents where every matching child satisfies `f`.
/// Vacuously true for parents with no children.
pub fn child_every(
    f: ::ruprizzle::Filter<super::bench_child::BenchChild>,
) -> ::ruprizzle::Filter<BenchGrandChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_children",
        child_col: "id",
        parent_table: "bench_grandchildren",
        parent_col: "child_id",
        filter: Box::new((!f).node),
        negated: true,
    })
}
/// Prisma-flavoured repository for `#model_name`.
#[derive(Debug, Clone, Copy)]
pub struct BenchGrandChildRepo<'a> {
    db: &'a super::Db,
}
impl<'a> BenchGrandChildRepo<'a> {
    /// Creates a new repository handle.
    pub(crate) fn new(db: &'a super::Db) -> Self {
        Self { db }
    }
    /// Start a `find_many` query.
    pub fn find_many(&self) -> ::ruprizzle::SelectQuery<'a, BenchGrandChild> {
        ::ruprizzle::SelectQuery::new(self.db.raw_pool())
    }
    /// Start an `insert` query.
    pub fn create(
        &self,
        _data: BenchGrandChildInsert,
    ) -> ::ruprizzle::InsertQuery<'a, BenchGrandChild> {
        let mut insert = ::ruprizzle::InsertQuery::new(self.db.raw_pool());
        insert = insert.set(ID, _data.id);
        insert = insert.set(CHILD_ID, _data.child_id);
        insert = insert.set(NAME, _data.name);
        insert
    }
    /// Start a multi-row `insert` query.
    pub fn create_many(
        &self,
        _data: Vec<BenchGrandChildInsert>,
    ) -> ::ruprizzle::InsertManyQuery<'a, BenchGrandChild> {
        let mut q = ::ruprizzle::InsertManyQuery::new(self.db.raw_pool());
        for _row in _data {
            q = q
                .row([
                    ("id", ::ruprizzle::Encodable::to_value(&_row.id)),
                    ("child_id", ::ruprizzle::Encodable::to_value(&_row.child_id)),
                    ("name", ::ruprizzle::Encodable::to_value(&_row.name)),
                ]);
        }
        q
    }
    /// Start an `update` query.
    pub fn update(&self) -> ::ruprizzle::UpdateQuery<'a, BenchGrandChild> {
        ::ruprizzle::UpdateQuery::new(self.db.raw_pool())
    }
    /// Start a `delete` query.
    pub fn delete(&self) -> ::ruprizzle::DeleteQuery<'a, BenchGrandChild> {
        ::ruprizzle::DeleteQuery::new(self.db.raw_pool())
    }
}
