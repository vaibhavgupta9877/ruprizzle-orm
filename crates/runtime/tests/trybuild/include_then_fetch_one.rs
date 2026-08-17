//! `.include(...).fetch_one()` is not available; use `.exec_one()` instead.

use ruprizzle::{Column, IncludeList, Model, Pool, Related, SelectQuery};

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[derive(sqlx::FromRow, Clone)]
struct Post {
    id: i64,
    author_id: i64,
}

impl Model for Post {
    const TABLE: &'static str = "posts";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

fn set_user_posts(_u: &mut User, _posts: Related<Vec<Post>>) {}

fn include_then_fetch_one(db: &Pool) {
    let include = IncludeList::new(
        |u: &User| u.id,
        set_user_posts,
        POST_AUTHOR_ID,
        |p: &Post| p.author_id,
    );
    let _ = SelectQuery::<User>::new(db).include(include).fetch_one();
}

fn main() {}
