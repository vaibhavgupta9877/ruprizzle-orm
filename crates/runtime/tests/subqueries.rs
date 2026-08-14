//! Subquery round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::sqlx::{self, ColumnIndex, Decode, Type};
use ruprizzle::{Column, Model, SelectQuery, Value};
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
    author_id: i64,
    published: bool,
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
    bool: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            author_id: row.try_get(2)?,
            published: row.try_get(3)?,
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
    const COLUMNS: &'static [&'static str] = &["id", "title", "author_id", "published"];
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
            author_id: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
            published: ::ruprizzle::rusqlite::get::<bool>(row, 3)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            title: row.get::<String>(1)?,
            author_id: row.get::<i64>(2)?,
            published: row.get::<bool>(3)?,
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
            author_id: row
                .try_get::<usize, i64>(2)
                .map_err(ruprizzle::Error::TokioPostgres)?,
            published: row
                .try_get::<usize, bool>(3)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");
const PUBLISHED: Column<Post, bool> = Column::new("posts", "published");

const SETUP_SQL: &str = r#"
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL
);
CREATE TABLE posts (
    id BIGINT PRIMARY KEY,
    title TEXT NOT NULL,
    author_id BIGINT NOT NULL,
    published BOOLEAN NOT NULL
);
INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Carol');
INSERT INTO posts (id, title, author_id, published) VALUES (10, 'First', 1, TRUE), (20, 'Second', 1, FALSE), (30, 'Third', 2, TRUE);
"#;

fn expected_in_subquery_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."id" IN (SELECT "posts"."author_id" FROM "posts" WHERE "posts"."published" = $1)"#
        }
        "sqlite" | "mysql" => {
            r#"SELECT `users`.`id`, `users`.`name` FROM `users` WHERE `users`.`id` IN (SELECT `posts`.`author_id` FROM `posts` WHERE `posts`.`published` = ?)"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

fn expected_not_in_subquery_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."id" NOT IN (SELECT "posts"."author_id" FROM "posts" WHERE "posts"."published" = $1)"#
        }
        "sqlite" | "mysql" => {
            r#"SELECT `users`.`id`, `users`.`name` FROM `users` WHERE `users`.`id` NOT IN (SELECT `posts`.`author_id` FROM `posts` WHERE `posts`.`published` = ?)"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn in_subquery_to_sql_and_fetch(db: TestDb) {
        let sub = SelectQuery::<Post>::new(db.pool())
            .columns(POST_AUTHOR_ID)
            .filter(PUBLISHED.eq(true));
        let q = SelectQuery::<User>::new(db.pool())
            .filter(USER_ID.in_subquery(sub));

        let compiled = q.to_sql();
        assert_eq!(compiled.sql, expected_in_subquery_sql(db.backend().as_str()));
        assert_eq!(compiled.binds, vec![Value::Bool(true)]);

        let mut rows = q.fetch_all().await?;
        rows.sort_by_key(|u| u.id);
        assert_eq!(
            rows,
            vec![
                User { id: 1, name: "Alice".into() },
                User { id: 2, name: "Bob".into() },
            ]
        );
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn not_in_subquery_fetch(db: TestDb) {
        let sub = SelectQuery::<Post>::new(db.pool())
            .columns(POST_AUTHOR_ID)
            .filter(PUBLISHED.eq(true));
        let q = SelectQuery::<User>::new(db.pool())
            .filter(USER_ID.not_in_subquery(sub));

        let compiled = q.to_sql();
        assert_eq!(compiled.sql, expected_not_in_subquery_sql(db.backend().as_str()));

        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![User { id: 3, name: "Carol".into() }]);
    }
}
