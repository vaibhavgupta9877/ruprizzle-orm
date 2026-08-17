//! Set-operation round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::sqlx::{self, ColumnIndex, Decode, Type};
use ruprizzle::{Column, Model, SelectQuery};
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
            author_id: row.try_get(2)?,
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
    const COLUMNS: &'static [&'static str] = &["id", "title", "author_id"];
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
        })
    }
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const POST_AUTHOR_ID: Column<Post, i64> = Column::new("posts", "author_id");

const SETUP_SQL: &str = r#"
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL
);
CREATE TABLE posts (
    id BIGINT PRIMARY KEY,
    title TEXT NOT NULL,
    author_id BIGINT NOT NULL
);
INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Carol');
INSERT INTO posts (id, title, author_id) VALUES (10, 'First', 1), (20, 'Second', 2), (30, 'Third', 4);
"#;

fn expected_union_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"(SELECT "users"."id" FROM "users") UNION (SELECT "posts"."author_id" FROM "posts")"#
        }
        "mysql" => {
            r#"(SELECT `users`.`id` FROM `users`) UNION (SELECT `posts`.`author_id` FROM `posts`)"#
        }
        "sqlite" => {
            r#"SELECT * FROM (SELECT `users`.`id` FROM `users`) AS __rz_l UNION SELECT * FROM (SELECT `posts`.`author_id` FROM `posts`) AS __rz_r"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

fn expected_union_all_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"(SELECT "users"."id" FROM "users") UNION ALL (SELECT "posts"."author_id" FROM "posts")"#
        }
        "mysql" => {
            r#"(SELECT `users`.`id` FROM `users`) UNION ALL (SELECT `posts`.`author_id` FROM `posts`)"#
        }
        "sqlite" => {
            r#"SELECT * FROM (SELECT `users`.`id` FROM `users`) AS __rz_l UNION ALL SELECT * FROM (SELECT `posts`.`author_id` FROM `posts`) AS __rz_r"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn union_to_sql_and_fetch(db: TestDb) {
        let q = SelectQuery::<User>::new(db.pool())
            .columns((USER_ID,))
            .union(SelectQuery::<Post>::new(db.pool()).columns((POST_AUTHOR_ID,)));

        let compiled = q.to_sql().unwrap();
        assert_eq!(compiled.sql, expected_union_sql(db.backend().as_str()));
        assert!(compiled.binds.is_empty());

        let mut rows: Vec<(i64,)> = q.fetch_all().await?;
        rows.sort();
        assert_eq!(rows, vec![(1,), (2,), (3,), (4,)]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn union_all_fetch(db: TestDb) {
        let q = SelectQuery::<User>::new(db.pool())
            .columns((USER_ID,))
            .union_all(SelectQuery::<Post>::new(db.pool()).columns((POST_AUTHOR_ID,)));

        let compiled = q.to_sql().unwrap();
        assert_eq!(compiled.sql, expected_union_all_sql(db.backend().as_str()));
        assert!(compiled.binds.is_empty());

        let mut rows: Vec<(i64,)> = q.fetch_all().await?;
        rows.sort();
        assert_eq!(rows, vec![(1,), (1,), (2,), (2,), (3,), (4,)]);
    }
}
