//! `Column<M, String>::eq` must reject a value that is not `Into<String>`.

use ruprizzle::{Column, Model};

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::FromValue::from_value(&row.0[0])?,
        })
    }
}

const EMAIL: Column<User, String> = Column::new("users", "email");

fn bad() {
    let _ = EMAIL.eq(42);
}

fn main() {}
