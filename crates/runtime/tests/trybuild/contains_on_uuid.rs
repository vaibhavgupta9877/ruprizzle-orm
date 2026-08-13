//! `.contains()` is only available on `String` columns, not `Uuid`.

use ruprizzle::{Column, Model};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct User {
    id: Uuid,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<Uuid>(row, 0)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<Uuid>(0)?,
        })
    }
}

const USER_ID: Column<User, Uuid> = Column::new("users", "id");

fn bad() {
    let _ = USER_ID.contains("x");
}

fn main() {}
