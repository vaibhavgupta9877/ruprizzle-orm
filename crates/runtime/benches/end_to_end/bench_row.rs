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
pub struct BenchRow {
    pub id: i64,
    pub name: String,
    pub n: i64,
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, AnyRow> for BenchRow {
    fn from_row(row: &'r AnyRow) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct::<i64>(row, "id")?,
            name: ::ruprizzle::decode::direct::<String>(row, "name")?,
            n: ::ruprizzle::decode::direct::<i64>(row, "n")?,
        })
    }
}
/// Insert shape: required fields are required, defaulted/optional fields
/// are `Option` so the database can fill them in.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchRowInsert {
    pub id: i64,
    pub name: String,
    pub n: i64,
}
/// Update shape: every field is `Option` to support explicit nulls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchRowUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
}
impl ::ruprizzle::Model for BenchRow {
    const TABLE: &'static str = "bench_rows";
    const PRIMARY_KEY: &'static str = "id";
}
/// Table name for this model.
pub const TABLE: &str = "bench_rows";
pub const ID: Column<BenchRow, i64> = Column::new("bench_rows", "id");
pub const NAME: Column<BenchRow, String> = Column::new("bench_rows", "name");
pub const N: Column<BenchRow, i64> = Column::new("bench_rows", "n");
/// Prisma-flavoured repository for `#model_name`.
#[derive(Debug, Clone, Copy)]
pub struct BenchRowRepo<'a> {
    db: &'a super::Db,
}
impl<'a> BenchRowRepo<'a> {
    /// Creates a new repository handle.
    pub(crate) fn new(db: &'a super::Db) -> Self {
        Self { db }
    }
    /// Start a `find_many` query.
    pub fn find_many(&self) -> ::ruprizzle::SelectQuery<'a, BenchRow> {
        ::ruprizzle::SelectQuery::new(self.db.raw_pool())
    }
    /// Start an `insert` query.
    pub fn create(
        &self,
        _data: BenchRowInsert,
    ) -> ::ruprizzle::InsertQuery<'a, BenchRow> {
        let mut insert = ::ruprizzle::InsertQuery::new(self.db.raw_pool());
        insert = insert.set(ID, _data.id);
        insert = insert.set(NAME, _data.name);
        insert = insert.set(N, _data.n);
        insert
    }
    /// Start a multi-row `insert` query.
    pub fn create_many(
        &self,
        _data: Vec<BenchRowInsert>,
    ) -> ::ruprizzle::InsertManyQuery<'a, BenchRow> {
        let mut q = ::ruprizzle::InsertManyQuery::new(self.db.raw_pool());
        for _row in _data {
            q = q
                .row([
                    ("id", ::ruprizzle::Encodable::to_value(&_row.id)),
                    ("name", ::ruprizzle::Encodable::to_value(&_row.name)),
                    ("n", ::ruprizzle::Encodable::to_value(&_row.n)),
                ]);
        }
        q
    }
    /// Start an `update` query.
    pub fn update(&self) -> ::ruprizzle::UpdateQuery<'a, BenchRow> {
        ::ruprizzle::UpdateQuery::new(self.db.raw_pool())
    }
    /// Start a `delete` query.
    pub fn delete(&self) -> ::ruprizzle::DeleteQuery<'a, BenchRow> {
        ::ruprizzle::DeleteQuery::new(self.db.raw_pool())
    }
}
