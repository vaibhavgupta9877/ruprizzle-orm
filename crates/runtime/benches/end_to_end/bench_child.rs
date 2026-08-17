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
pub struct BenchChild {
    pub id: i64,
    pub parent_id: i64,
    pub name: String,
    #[serde(skip_serializing_if = "::ruprizzle::Related::is_absent", default)]
    pub parent: ::ruprizzle::Related<Option<super::bench_parent::BenchParent>>,
    #[serde(skip_serializing_if = "::ruprizzle::Related::is_absent", default)]
    pub grandchildren: ::ruprizzle::Related<
        Vec<super::bench_grand_child::BenchGrandChild>,
    >,
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, AnyRow> for BenchChild {
    fn from_row(row: &'r AnyRow) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            parent_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::postgres::PgRow>
for BenchChild {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::postgres::PgRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            parent_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::sqlite::SqliteRow>
for BenchChild {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::sqlite::SqliteRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            parent_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::mysql::MySqlRow>
for BenchChild {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::mysql::MySqlRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            parent_id: ::ruprizzle::decode::direct_idx(row, 1)?,
            name: ::ruprizzle::decode::direct_idx(row, 2)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromRusqliteRow for BenchChild {
    fn from_rusqlite_row(
        row: &::ruprizzle::rusqlite::RusqliteRow,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get_i64(row, 0)?,
            parent_id: ::ruprizzle::rusqlite::get_i64(row, 1)?,
            name: ::ruprizzle::rusqlite::get_text(row, 2)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromOwnedRow for BenchChild {
    fn from_owned_row(
        row: &::ruprizzle::rusqlite::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::Row::get::<i64>(row, 0)?,
            parent_id: ::ruprizzle::rusqlite::Row::get::<i64>(row, 1)?,
            name: ::ruprizzle::rusqlite::Row::get::<String>(row, 2)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
#[cfg(feature = "postgres-tokio-postgres")]
impl ::ruprizzle::tokio_postgres::FromTokioPostgresRow for BenchChild {
    fn from_tokio_postgres_row(
        row: &::ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: row.try_get::<usize, i64>(0).map_err(::ruprizzle::Error::TokioPostgres)?,
            parent_id: row
                .try_get::<usize, i64>(1)
                .map_err(::ruprizzle::Error::TokioPostgres)?,
            name: row
                .try_get::<usize, String>(2)
                .map_err(::ruprizzle::Error::TokioPostgres)?,
            parent: ::ruprizzle::Related::default(),
            grandchildren: ::ruprizzle::Related::default(),
        })
    }
}
/// Insert shape: required fields are required, defaulted/optional fields
/// are `Option` so the database can fill them in.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchChildInsert {
    pub id: i64,
    pub parent_id: i64,
    pub name: String,
}
/// Update shape: every field is `Option` to support explicit nulls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchChildUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl ::ruprizzle::Model for BenchChild {
    const TABLE: &'static str = "bench_children";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "parent_id", "name"];
}
/// Table name for this model.
pub const TABLE: &str = "bench_children";
pub const ID: Column<BenchChild, i64> = Column::new("bench_children", "id");
pub const PARENT_ID: Column<BenchChild, i64> = Column::new(
    "bench_children",
    "parent_id",
);
pub const NAME: Column<BenchChild, String> = Column::new("bench_children", "name");
/// Returns an `IncludeOne` for this relation.
pub fn parent() -> ::ruprizzle::IncludeOne<
    'static,
    BenchChild,
    super::bench_parent::BenchParent,
    i64,
    (),
> {
    ::ruprizzle::IncludeOne::new(
        |__row| __row.parent_id,
        |__row, __loaded| __row.parent = __loaded,
        super::bench_parent::ID,
        |__child| __child.id,
    )
}
/// Returns an `IncludeList` for this relation.
pub fn grandchildren() -> ::ruprizzle::IncludeList<
    'static,
    BenchChild,
    super::bench_grand_child::BenchGrandChild,
    i64,
    (),
> {
    ::ruprizzle::IncludeList::new(
        |__row| __row.id,
        |__row, __loaded| __row.grandchildren = __loaded,
        super::bench_grand_child::CHILD_ID,
        |__child| __child.child_id,
    )
}
/// Returns a filter for parents that have at least one matching child.
pub fn parent_some(
    f: ::ruprizzle::Filter<super::bench_parent::BenchParent>,
) -> ::ruprizzle::Filter<BenchChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_parents",
        child_col: "id",
        parent_table: "bench_children",
        parent_col: "parent_id",
        filter: Box::new(f.node),
        negated: false,
    })
}
/// Returns a filter for parents that have no matching child.
pub fn parent_none(
    f: ::ruprizzle::Filter<super::bench_parent::BenchParent>,
) -> ::ruprizzle::Filter<BenchChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_parents",
        child_col: "id",
        parent_table: "bench_children",
        parent_col: "parent_id",
        filter: Box::new(f.node),
        negated: true,
    })
}
/// Returns a filter for parents where every matching child satisfies `f`.
/// Vacuously true for parents with no children.
pub fn parent_every(
    f: ::ruprizzle::Filter<super::bench_parent::BenchParent>,
) -> ::ruprizzle::Filter<BenchChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_parents",
        child_col: "id",
        parent_table: "bench_children",
        parent_col: "parent_id",
        filter: Box::new((!f).node),
        negated: true,
    })
}
/// Returns a filter for parents that have at least one matching child.
pub fn grandchildren_some(
    f: ::ruprizzle::Filter<super::bench_grand_child::BenchGrandChild>,
) -> ::ruprizzle::Filter<BenchChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_grandchildren",
        child_col: "child_id",
        parent_table: "bench_children",
        parent_col: "id",
        filter: Box::new(f.node),
        negated: false,
    })
}
/// Returns a filter for parents that have no matching child.
pub fn grandchildren_none(
    f: ::ruprizzle::Filter<super::bench_grand_child::BenchGrandChild>,
) -> ::ruprizzle::Filter<BenchChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_grandchildren",
        child_col: "child_id",
        parent_table: "bench_children",
        parent_col: "id",
        filter: Box::new(f.node),
        negated: true,
    })
}
/// Returns a filter for parents where every matching child satisfies `f`.
/// Vacuously true for parents with no children.
pub fn grandchildren_every(
    f: ::ruprizzle::Filter<super::bench_grand_child::BenchGrandChild>,
) -> ::ruprizzle::Filter<BenchChild> {
    ::ruprizzle::Filter::new(::ruprizzle::FilterNode::Exists {
        child_table: "bench_grandchildren",
        child_col: "child_id",
        parent_table: "bench_children",
        parent_col: "id",
        filter: Box::new((!f).node),
        negated: true,
    })
}
/// Prisma-flavoured repository for `#model_name`.
#[derive(Debug, Clone, Copy)]
pub struct BenchChildRepo<'a> {
    db: &'a super::Db,
}
impl<'a> BenchChildRepo<'a> {
    /// Creates a new repository handle.
    pub(crate) fn new(db: &'a super::Db) -> Self {
        Self { db }
    }
    /// Start a `find_many` query.
    pub fn find_many(&self) -> ::ruprizzle::SelectQuery<'a, BenchChild> {
        ::ruprizzle::SelectQuery::new(self.db.raw_pool())
    }
    /// Start an `insert` query.
    pub fn create(
        &self,
        _data: BenchChildInsert,
    ) -> ::ruprizzle::InsertQuery<'a, BenchChild> {
        let mut insert = ::ruprizzle::InsertQuery::new(self.db.raw_pool());
        insert = insert.set(ID, _data.id);
        insert = insert.set(PARENT_ID, _data.parent_id);
        insert = insert.set(NAME, _data.name);
        insert
    }
    /// Start a multi-row `insert` query.
    pub fn create_many(
        &self,
        _data: Vec<BenchChildInsert>,
    ) -> ::ruprizzle::InsertManyQuery<'a, BenchChild> {
        let mut q = ::ruprizzle::InsertManyQuery::new(self.db.raw_pool());
        for _row in _data {
            q = q
                .row([
                    ("id", ::ruprizzle::Encodable::to_value(&_row.id)),
                    ("parent_id", ::ruprizzle::Encodable::to_value(&_row.parent_id)),
                    ("name", ::ruprizzle::Encodable::to_value(&_row.name)),
                ]);
        }
        q
    }
    /// Start an `update` query.
    pub fn update(&self) -> ::ruprizzle::UpdateQuery<'a, BenchChild> {
        ::ruprizzle::UpdateQuery::new(self.db.raw_pool())
    }
    /// Start a `delete` query.
    pub fn delete(&self) -> ::ruprizzle::DeleteQuery<'a, BenchChild> {
        ::ruprizzle::DeleteQuery::new(self.db.raw_pool())
    }
}
