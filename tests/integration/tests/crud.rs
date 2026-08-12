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

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
}

const ID: Column<Task, i64> = Column::new("tasks", "id");
const NAME: Column<Task, String> = Column::new("tasks", "name");

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn runtime_crud_round_trip(db: TestDb) {
        let pool = db.pool();

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

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn runtime_upsert_round_trip(db: TestDb) {
        let pool = db.pool();

        let one: Task = InsertQuery::<Task>::new(pool)
            .set(ID, 1)
            .set(NAME, "first")
            .on_conflict(["id"])
            .do_update(["name"])
            .exec()
            .await?;
        assert_eq!(one.name, "first");

        let two: Task = InsertQuery::<Task>::new(pool)
            .set(ID, 1)
            .set(NAME, "second")
            .on_conflict(["id"])
            .do_update(["name"])
            .exec()
            .await?;
        assert_eq!(two.id, 1);
        assert_eq!(two.name, "second");

        let row = SelectQuery::<Task>::new(pool)
            .filter(ID.eq(1))
            .fetch_one()
            .await?;
        assert_eq!(row.name, "second");
        assert_eq!(SelectQuery::<Task>::new(pool).count().await?, 1);
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn runtime_insert_many_round_trip(db: TestDb) {
        let pool = db.pool();

        let rows: Vec<Task> = ruprizzle::InsertManyQuery::<Task>::new(pool)
            .row([("id", ruprizzle::Value::I64(1)), ("name", ruprizzle::Value::Str("one".into()))])
            .row([("id", ruprizzle::Value::I64(2)), ("name", ruprizzle::Value::Str("two".into()))])
            .row([("id", ruprizzle::Value::I64(3)), ("name", ruprizzle::Value::Str("three".into()))])
            .exec()
            .await?;

        assert_eq!(rows.len(), 3);
        assert_eq!(SelectQuery::<Task>::new(pool).count().await?, 3);

        let names: Vec<String> = SelectQuery::<Task>::new(pool)
            .columns((NAME,))
            .fetch_all()
            .await?
            .into_iter()
            .map(|(n,)| n)
            .collect();

        assert!(names.contains(&"one".to_string()));
        assert!(names.contains(&"two".to_string()));
        assert!(names.contains(&"three".to_string()));
    }
}

both_dbs! {
    setup = "CREATE TABLE tasks (id BIGINT PRIMARY KEY, name TEXT NOT NULL)";
    async fn runtime_pagination_round_trip(db: TestDb) {
        let pool = db.pool();

        for i in 1..=5 {
            InsertQuery::<Task>::new(pool)
                .set(ID, i)
                .set(NAME, format!("task-{i}"))
                .exec()
                .await?;
        }

        let page: Vec<Task> = SelectQuery::<Task>::new(pool)
            .order_by(ID.asc())
            .offset(1)
            .limit(2)
            .fetch_all()
            .await?;
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, 2);
        assert_eq!(page[1].id, 3);

        let after: Vec<Task> = SelectQuery::<Task>::new(pool)
            .after(ID, 2, 2)
            .fetch_all()
            .await?;
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].id, 3);
        assert_eq!(after[1].id, 4);

        let before: Vec<Task> = SelectQuery::<Task>::new(pool)
            .before(ID, 5, 2)
            .fetch_all()
            .await?;
        assert_eq!(before.len(), 2);
        assert_eq!(before[0].id, 4);
        assert_eq!(before[1].id, 3);
    }
}
