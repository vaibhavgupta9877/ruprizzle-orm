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
pub struct BenchBulk {
    pub id: i64,
    pub name: String,
    pub n: i64,
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, AnyRow> for BenchBulk {
    fn from_row(row: &'r AnyRow) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            n: ::ruprizzle::decode::direct_idx(row, 2)?,
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::postgres::PgRow>
for BenchBulk {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::postgres::PgRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            n: ::ruprizzle::decode::direct_idx(row, 2)?,
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::sqlite::SqliteRow>
for BenchBulk {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::sqlite::SqliteRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            n: ::ruprizzle::decode::direct_idx(row, 2)?,
        })
    }
}
impl<'r> ::ruprizzle::sqlx::FromRow<'r, ::ruprizzle::sqlx::mysql::MySqlRow>
for BenchBulk {
    fn from_row(
        row: &'r ::ruprizzle::sqlx::mysql::MySqlRow,
    ) -> Result<Self, ::ruprizzle::sqlx::Error> {
        Ok(Self {
            id: ::ruprizzle::decode::direct_idx(row, 0)?,
            name: ::ruprizzle::decode::direct_idx(row, 1)?,
            n: ::ruprizzle::decode::direct_idx(row, 2)?,
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromRusqliteRow for BenchBulk {
    fn from_rusqlite_row(
        row: &::ruprizzle::rusqlite::RusqliteRow,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get_i64(row, 0)?,
            name: ::ruprizzle::rusqlite::get_text(row, 1)?,
            n: ::ruprizzle::rusqlite::get_i64(row, 2)?,
        })
    }
}
#[cfg(feature = "sqlite-rusqlite")]
impl ::ruprizzle::rusqlite::FromOwnedRow for BenchBulk {
    fn from_owned_row(
        row: &::ruprizzle::rusqlite::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::Row::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::Row::get::<String>(row, 1)?,
            n: ::ruprizzle::rusqlite::Row::get::<i64>(row, 2)?,
        })
    }
}
#[cfg(feature = "postgres-tokio-postgres")]
impl ::ruprizzle::tokio_postgres::FromTokioPostgresRow for BenchBulk {
    fn from_tokio_postgres_row(
        row: &::ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ::ruprizzle::Error> {
        Ok(Self {
            id: row.try_get::<usize, i64>(0).map_err(::ruprizzle::Error::TokioPostgres)?,
            name: row
                .try_get::<usize, String>(1)
                .map_err(::ruprizzle::Error::TokioPostgres)?,
            n: row.try_get::<usize, i64>(2).map_err(::ruprizzle::Error::TokioPostgres)?,
        })
    }
}
/// Insert shape: required fields are required, defaulted/optional fields
/// are `Option` so the database can fill them in.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchBulkInsert {
    pub id: i64,
    pub name: String,
    pub n: i64,
}
/// Update shape: every field is `Option` to support explicit nulls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(crate = "::ruprizzle::serde")]
pub struct BenchBulkUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
}
impl ::ruprizzle::Model for BenchBulk {
    const TABLE: &'static str = "bench_bulk";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name", "n"];
}
/// Table name for this model.
pub const TABLE: &str = "bench_bulk";
pub const ID: Column<BenchBulk, i64> = Column::new("bench_bulk", "id");
pub const NAME: Column<BenchBulk, String> = Column::new("bench_bulk", "name");
pub const N: Column<BenchBulk, i64> = Column::new("bench_bulk", "n");
/// Prisma-flavoured repository for `#model_name`.
#[derive(Debug, Clone, Copy)]
pub struct BenchBulkRepo<'a> {
    db: &'a super::Db,
}
impl<'a> BenchBulkRepo<'a> {
    /// Creates a new repository handle.
    pub(crate) fn new(db: &'a super::Db) -> Self {
        Self { db }
    }
    /// Start a `find_many` query.
    pub fn find_many(&self) -> ::ruprizzle::SelectQuery<'a, BenchBulk> {
        ::ruprizzle::SelectQuery::new(self.db.raw_pool())
    }
    /// Start an `insert` query.
    pub fn create(
        &self,
        _data: BenchBulkInsert,
    ) -> ::ruprizzle::InsertQuery<'a, BenchBulk> {
        let mut insert = ::ruprizzle::InsertQuery::new(self.db.raw_pool());
        insert = insert.set(ID, _data.id);
        insert = insert.set(NAME, _data.name);
        insert = insert.set(N, _data.n);
        insert
    }
    /// Start a multi-row `insert` query.
    pub fn create_many(
        &self,
        _data: Vec<BenchBulkInsert>,
    ) -> ::ruprizzle::InsertManyQuery<'a, BenchBulk> {
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
    pub fn update(&self) -> ::ruprizzle::UpdateQuery<'a, BenchBulk> {
        ::ruprizzle::UpdateQuery::new(self.db.raw_pool())
    }
    /// Start a `delete` query.
    pub fn delete(&self) -> ::ruprizzle::DeleteQuery<'a, BenchBulk> {
        ::ruprizzle::DeleteQuery::new(self.db.raw_pool())
    }
}
