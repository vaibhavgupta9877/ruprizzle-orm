use ruprizzle::{Column, DeleteQuery, Model, Pool};

#[derive(sqlx::FromRow)]
struct Task {
    id: i64,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");

fn delete_by_user(db: &Pool) {
    let _ = DeleteQuery::<Task>::new(db).filter(USER_ID.eq(1)).exec();
}

fn main() {}
