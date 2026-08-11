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

const USER_ID: Column<User, Uuid> = Column::new("users", "id");

fn bad() {
    let _ = USER_ID.contains("x");
}

fn main() {}
