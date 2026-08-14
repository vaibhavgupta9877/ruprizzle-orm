//! Self-join round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::sqlx::{self, ColumnIndex, Decode, Row, Type};
use ruprizzle::{Column, Join2, JoinSide, LeftJoin2, Maybe, Model, SelectQuery};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq)]
struct Employee {
    id: i64,
    name: String,
    manager_id: i64,
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

impl Model for Employee {
    const TABLE: &'static str = "employees";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name", "manager_id"];
}

impl<R: Row> JoinSide<R> for Employee
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
            manager_id: row.try_get(2)?,
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

const EMPLOYEE_ID: Column<Employee, i64> = Column::new("employees", "id");
const MANAGER_ID: Column<Employee, i64> = Column::new("employees", "manager_id");

const SETUP_SQL: &str = r#"
CREATE TABLE employees (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    manager_id BIGINT NOT NULL
);
INSERT INTO employees (id, name, manager_id) VALUES
(1, 'CEO', 0),
(2, 'Alice', 1),
(3, 'Bob', 1);
"#;

fn expected_self_join_sql(backend: &str) -> &'static str {
    match backend {
        "postgres" => {
            r#"SELECT "employees"."id", "employees"."name", "employees"."manager_id", "m"."id", "m"."name", "m"."manager_id" FROM "employees" INNER JOIN "employees" AS "m" ON "employees"."manager_id" = "m"."id""#
        }
        "sqlite" => {
            r#"SELECT `employees`.`id`, `employees`.`name`, `employees`.`manager_id`, `m`.`id`, `m`.`name`, `m`.`manager_id` FROM `employees` INNER JOIN `employees` AS `m` ON `employees`.`manager_id` = `m`.`id`"#
        }
        "mysql" => {
            r#"SELECT `employees`.`id`, `employees`.`name`, `employees`.`manager_id`, `m`.`id`, `m`.`name`, `m`.`manager_id` FROM `employees` INNER JOIN `employees` AS `m` ON `employees`.`manager_id` = `m`.`id`"#
        }
        _ => panic!("unknown backend {backend}"),
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn self_inner_join_to_sql_and_fetch(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .inner_join_aliased::<Employee>("m", MANAGER_ID.on(EMPLOYEE_ID.aliased("m")));
        let compiled = q.to_sql().unwrap();
        assert_eq!(compiled.sql, expected_self_join_sql(db.backend().as_str()));

        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![
            Join2(
                Employee { id: 2, name: "Alice".into(), manager_id: 1 },
                Employee { id: 1, name: "CEO".into(), manager_id: 0 },
            ),
            Join2(
                Employee { id: 3, name: "Bob".into(), manager_id: 1 },
                Employee { id: 1, name: "CEO".into(), manager_id: 0 },
            ),
        ]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn self_left_join_handles_unmatched_side(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .left_join_aliased::<Employee>("m", MANAGER_ID.on(EMPLOYEE_ID.aliased("m")));

        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![
            LeftJoin2(
                Employee { id: 1, name: "CEO".into(), manager_id: 0 },
                Maybe(None),
            ),
            LeftJoin2(
                Employee { id: 2, name: "Alice".into(), manager_id: 1 },
                Maybe(Some(Employee { id: 1, name: "CEO".into(), manager_id: 0 })),
            ),
            LeftJoin2(
                Employee { id: 3, name: "Bob".into(), manager_id: 1 },
                Maybe(Some(Employee { id: 1, name: "CEO".into(), manager_id: 0 })),
            ),
        ]);
    }
}
