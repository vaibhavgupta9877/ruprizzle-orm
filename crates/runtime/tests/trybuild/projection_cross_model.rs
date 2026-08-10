//! `SelectQuery::<User>::columns` must reject a `Column<Post, _>` projection.

use ruprizzle::{Column, Model, Pool, SelectQuery};

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(sqlx::FromRow)]
struct Post {
    id: i64,
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

const POST_ID: Column<Post, i64> = Column::new("posts", "id");

fn cross_model(db: &Pool) {
    let _ = SelectQuery::<User>::new(db).columns(POST_ID).fetch_one();
}

fn main() {}
