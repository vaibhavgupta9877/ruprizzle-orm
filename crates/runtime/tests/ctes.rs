//! CTE round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::sqlx::{self, ColumnIndex, Decode, Type};
use ruprizzle::{Column, Filter, Model, SelectQuery, Value};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Manager {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Employee {
    id: i64,
    name: String,
    manager_id: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct Reports {
    id: i64,
    name: String,
    manager_id: i64,
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

impl<'r, R: sqlx::Row> sqlx::FromRow<'r, R> for Manager
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

impl<'r, R: sqlx::Row> sqlx::FromRow<'r, R> for Employee
where
    usize: ColumnIndex<R>,
    i64: for<'a> Decode<'a, R::Database> + Type<R::Database>,
    String: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            name: row.try_get(1)?,
            manager_id: row.try_get(2)?,
        })
    }
}

impl<'r, R: sqlx::Row> sqlx::FromRow<'r, R> for Reports
where
    usize: ColumnIndex<R>,
    i64: for<'a> Decode<'a, R::Database> + Type<R::Database>,
    String: for<'a> Decode<'a, R::Database> + Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get(0)?,
            name: row.try_get(1)?,
            manager_id: row.try_get(2)?,
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
impl ruprizzle::rusqlite::FromRusqliteRow for Manager {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Manager {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Employee {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            manager_id: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Employee {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
            manager_id: row.get::<i64>(2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Reports {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            manager_id: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Reports {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
            manager_id: row.get::<i64>(2)?,
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
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Manager {
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
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Employee {
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
            manager_id: row
                .try_get::<usize, i64>(2)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Reports {
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
            manager_id: row
                .try_get::<usize, i64>(2)
                .map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name"];
}

impl Model for Manager {
    const TABLE: &'static str = "managers";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name"];
}

impl Model for Employee {
    const TABLE: &'static str = "employees";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name", "manager_id"];
}

impl Model for Reports {
    const TABLE: &'static str = "reports";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name", "manager_id"];
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_NAME: Column<User, String> = Column::new("users", "name");
const ROLE: Column<User, String> = Column::new("users", "role");

const MANAGER_ID: Column<Manager, i64> = Column::new("managers", "id");

const EMPLOYEE_ID: Column<Employee, i64> = Column::new("employees", "id");
const REPORTS_MANAGER_ID: Column<Reports, i64> = Column::new("reports", "manager_id");

const SETUP_SQL: &str = r#"
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL
);
INSERT INTO users (id, name, role) VALUES
    (1, 'Alice', 'manager'),
    (2, 'Bob', 'employee'),
    (3, 'Carol', 'manager');
"#;

fn expected_cte_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"WITH "managers" AS (SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."role" = $1) SELECT "users"."id", "users"."name" FROM "users" WHERE EXISTS (SELECT "managers"."id", "managers"."name" FROM "managers" WHERE "managers"."id" = "users"."id")"#
        }
        "sqlite" | "mysql" => {
            r#"WITH `managers` AS (SELECT `users`.`id`, `users`.`name` FROM `users` WHERE `users`.`role` = ?) SELECT `users`.`id`, `users`.`name` FROM `users` WHERE EXISTS (SELECT `managers`.`id`, `managers`.`name` FROM `managers` WHERE `managers`.`id` = `users`.`id`)"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn cte_to_sql_and_fetch(db: TestDb) {
        let managers = SelectQuery::<User>::new(db.pool())
            .filter(ROLE.eq("manager"))
            .columns((USER_ID, USER_NAME));
        let q = SelectQuery::<User>::new(db.pool())
            .with("managers", managers)
            .filter(Filter::<User>::exists(
                SelectQuery::<Manager>::new(db.pool())
                    .filter(MANAGER_ID.correlated_to(USER_ID)),
            ));

        let compiled = q.to_sql().unwrap();
        assert_eq!(compiled.sql, expected_cte_sql(db.backend().as_str()));
        assert_eq!(compiled.binds, vec![Value::Str("manager".into())]);

        let mut rows = q.fetch_all().await?;
        rows.sort_by_key(|u| u.id);
        assert_eq!(
            rows,
            vec![
                User { id: 1, name: "Alice".into() },
                User { id: 3, name: "Carol".into() },
            ]
        );
    }
}

fn expected_recursive_prefix(backend: &str) -> &'static str {
    match backend {
        "postgres" => r#"WITH RECURSIVE "reports" AS ("#,
        "sqlite" | "mysql" => r#"WITH RECURSIVE `reports` AS ("#,
        _ => panic!("unknown backend {backend}"),
    }
}

both_dbs! {
    setup = "";
    async fn recursive_cte_to_sql(db: TestDb) {
        let anchor = SelectQuery::<Employee>::new(db.pool())
            .filter(EMPLOYEE_ID.eq(2));
        let recursive = SelectQuery::<Employee>::new(db.pool())
            .filter(Filter::<Employee>::exists(
                SelectQuery::<Reports>::new(db.pool())
                    .filter(REPORTS_MANAGER_ID.correlated_to(EMPLOYEE_ID)),
            ));
        let q = SelectQuery::<Reports>::new(db.pool())
            .with_recursive("reports", anchor, recursive);

        let compiled = q.to_sql().unwrap();
        assert!(compiled.sql.starts_with(expected_recursive_prefix(db.backend().as_str())));
        assert!(compiled.sql.contains("UNION ALL"));
        assert_eq!(compiled.binds, vec![Value::I64(2)]);
    }
}
