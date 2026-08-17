//! Explicit join round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::sqlx::{self, ColumnIndex, Decode, Row, Type};
use ruprizzle::{Column, Join2, JoinSide, LeftJoin2, Maybe, Model, SelectQuery};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Post {
    id: i64,
    title: String,
    user_id: i64,
}

impl<'r, R: sqlx::Row> sqlx::FromRow<'r, R> for User
where
    usize: ColumnIndex<R>,
    i64: for<'a> Decode<'a, R::Database> + Type<R::Database>,
    String: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            name: row.try_get(1)?,
        })
    }
}

impl<'r, R: sqlx::Row> sqlx::FromRow<'r, R> for Post
where
    usize: ColumnIndex<R>,
    i64: for<'a> Decode<'a, R::Database> + Type<R::Database>,
    String: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            user_id: row.try_get(2)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name"];
}

impl Model for Post {
    const TABLE: &'static str = "posts";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "title", "user_id"];
}

impl<R: Row> JoinSide<R> for User
where
    usize: ColumnIndex<R>,
    i64: for<'a> Decode<'a, R::Database> + Type<R::Database>,
    String: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_offset_row<'r>(
        row: &ruprizzle::OffsetRow<'r, R>,
    ) -> Result<Self, ruprizzle::sqlx::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            id: row.try_get(0)?,
            name: row.try_get(1)?,
        })
    }
}

impl<R: Row> JoinSide<R> for Post
where
    usize: ColumnIndex<R>,
    i64: for<'a> Decode<'a, R::Database> + Type<R::Database>,
    String: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_offset_row<'r>(
        row: &ruprizzle::OffsetRow<'r, R>,
    ) -> Result<Self, ruprizzle::sqlx::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            user_id: row.try_get(2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            user_id: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            title: row.get::<String>(1)?,
            user_id: row.get::<i64>(2)?,
        })
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for User {
    fn from_tokio_postgres_row(
        row: &ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row
                .try_get::<usize, i64>(0)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            name: row
                .try_get::<usize, String>(1)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Post {
    fn from_tokio_postgres_row(
        row: &ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row
                .try_get::<usize, i64>(0)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            title: row
                .try_get::<usize, String>(1)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            user_id: row
                .try_get::<usize, i64>(2)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const POST_USER_ID: Column<Post, i64> = Column::new("posts", "user_id");

const SETUP_SQL: &str = r#"
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL
);
CREATE TABLE posts (
    id BIGINT PRIMARY KEY,
    title TEXT NOT NULL,
    user_id BIGINT NOT NULL
);
INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');
INSERT INTO posts (id, title, user_id) VALUES (10, 'First', 1), (20, 'Second', 1);
"#;

fn expected_inner_join_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"SELECT "users"."id", "users"."name", "posts"."id", "posts"."title", "posts"."user_id" FROM "users" INNER JOIN "posts" ON "users"."id" = "posts"."user_id""#
        }
        "sqlite" => {
            r#"SELECT `users`.`id`, `users`.`name`, `posts`.`id`, `posts`.`title`, `posts`.`user_id` FROM `users` INNER JOIN `posts` ON `users`.`id` = `posts`.`user_id`"#
        }
        "mysql" => {
            r#"SELECT `users`.`id`, `users`.`name`, `posts`.`id`, `posts`.`title`, `posts`.`user_id` FROM `users` INNER JOIN `posts` ON `users`.`id` = `posts`.`user_id`"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

fn expected_left_join_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"SELECT "users"."id", "users"."name", "posts"."id", "posts"."title", "posts"."user_id" FROM "users" LEFT JOIN "posts" ON "users"."id" = "posts"."user_id""#
        }
        "sqlite" => {
            r#"SELECT `users`.`id`, `users`.`name`, `posts`.`id`, `posts`.`title`, `posts`.`user_id` FROM `users` LEFT JOIN `posts` ON `users`.`id` = `posts`.`user_id`"#
        }
        "mysql" => {
            r#"SELECT `users`.`id`, `users`.`name`, `posts`.`id`, `posts`.`title`, `posts`.`user_id` FROM `users` LEFT JOIN `posts` ON `users`.`id` = `posts`.`user_id`"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn inner_join_to_sql_and_fetch(db: TestDb) {
        let q = SelectQuery::<User>::new(db.pool())
            .inner_join::<Post>(USER_ID.on(POST_USER_ID));
        let compiled = q.to_sql().unwrap();
        assert_eq!(compiled.sql, expected_inner_join_sql(db.backend().as_str()));

        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![
            Join2(
                User { id: 1, name: "Alice".into() },
                Post { id: 10, title: "First".into(), user_id: 1 },
            ),
            Join2(
                User { id: 1, name: "Alice".into() },
                Post { id: 20, title: "Second".into(), user_id: 1 },
            ),
        ]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn left_join_to_sql_and_fetch(db: TestDb) {
        let q = SelectQuery::<User>::new(db.pool())
            .left_join::<Post>(USER_ID.on(POST_USER_ID));
        let compiled = q.to_sql().unwrap();
        assert_eq!(compiled.sql, expected_left_join_sql(db.backend().as_str()));

        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![
            LeftJoin2(
                User { id: 1, name: "Alice".into() },
                Maybe(Some(Post { id: 10, title: "First".into(), user_id: 1 })),
            ),
            LeftJoin2(
                User { id: 1, name: "Alice".into() },
                Maybe(Some(Post { id: 20, title: "Second".into(), user_id: 1 })),
            ),
            LeftJoin2(
                User { id: 2, name: "Bob".into() },
                Maybe(None),
            ),
        ]);
    }
}
