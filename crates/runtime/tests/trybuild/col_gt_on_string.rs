//! Ordering comparisons are gated to `Ordered`, which `String` deliberately
//! does not implement: lexicographic `>` on text is almost always a bug.

use ruprizzle::{Column, Model};

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const EMAIL: Column<User, String> = Column::new("users", "email");

fn bad() {
    let _ = EMAIL.gt("a");
}

fn main() {}
