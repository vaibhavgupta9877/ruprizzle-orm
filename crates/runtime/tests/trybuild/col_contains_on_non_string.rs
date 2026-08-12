//! String pattern operators exist only on `Column<M, String>`.

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

const ID: Column<User, i64> = Column::new("users", "id");

fn bad() {
    let _ = ID.contains("x");
}

fn main() {}
