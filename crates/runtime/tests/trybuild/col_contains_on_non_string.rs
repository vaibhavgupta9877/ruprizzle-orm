//! String pattern operators exist only on `Column<M, String>`.

use ruprizzle::{Column, Model};

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const ID: Column<User, i64> = Column::new("users", "id");

fn bad() {
    let _ = ID.contains("x");
}

fn main() {}
