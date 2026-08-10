//! Runtime CRUD round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::{Column, DeleteQuery, InsertQuery, Model, SelectQuery, UpdateQuery};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct Task {
    id: i64,
    name: String,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

const ID: Column<Task, i64> = Column::new("tasks", "id");
const NAME: Column<Task, String> = Column::new("tasks", "name");

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn runtime_crud_round_trip(db: TestDb) {
        let pool = db.any_pool();

        // INSERT
        let one: Task = InsertQuery::<Task>::new(pool)
            .set(ID, 1)
            .set(NAME, "first")
            .exec()
            .await?;

        let two: Task = InsertQuery::<Task>::new(pool)
            .set(ID, 2)
            .set(NAME, "second")
            .exec()
            .await?;

        assert_eq!(one.id, 1);
        assert_eq!(two.id, 2);

        // SELECT with filter
        let rows: Vec<Task> = SelectQuery::<Task>::new(pool)
            .filter(NAME.eq("first"))
            .fetch_all()
            .await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);

        // Projection
        let names: Vec<(String,)> = SelectQuery::<Task>::new(pool)
            .columns((NAME,))
            .fetch_all()
            .await?;
        assert_eq!(names.len(), 2);

        // UPDATE
        let affected = UpdateQuery::<Task>::new(pool)
            .filter(ID.eq(1))
            .set(NAME, "updated")
            .exec()
            .await?;
        assert_eq!(affected, 1);

        let updated = SelectQuery::<Task>::new(pool)
            .filter(ID.eq(1))
            .fetch_optional()
            .await?;
        assert_eq!(updated.unwrap().name, "updated");

        // DELETE
        let deleted = DeleteQuery::<Task>::new(pool)
            .filter(ID.eq(2))
            .exec()
            .await?;
        assert_eq!(deleted, 1);

        // COUNT
        let count = SelectQuery::<Task>::new(pool).count().await?;
        assert_eq!(count, 1);
    }
}
