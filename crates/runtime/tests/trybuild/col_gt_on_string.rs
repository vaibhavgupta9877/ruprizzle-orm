//! Ordering comparisons are gated to `Ordered`. `String` deliberately does not
//! implement it, because lexicographic `>` on text is almost always a bug.
//!
//! The column uses a local type rather than `String`, which has the same gate.
//! With `String`, rustc's diagnostic quotes `alloc/src/string.rs` only when the
//! toolchain has `rust-src` installed, so the snapshot would differ from machine
//! to machine.

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
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
        })
    }
}

/// Stands in for `String`: a column value type with no `Ordered` impl.
struct Email;

const EMAIL: Column<User, Email> = Column::new("users", "email");

fn bad() {
    let _ = EMAIL.gt("a");
}

fn main() {}
