use ruprizzle::{Column, DeleteQuery, Model, Pool};

#[derive(sqlx::FromRow)]
struct Task {
    id: i64,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

fn delete_all(db: &Pool) {
    let _ = DeleteQuery::<Task>::new(db).exec();
}

fn main() {}
