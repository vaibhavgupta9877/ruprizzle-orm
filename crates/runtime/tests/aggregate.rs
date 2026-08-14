//! Aggregate query round-trips over the dual-database `both_dbs!` harness.

use ruprizzle::{Column, Model, SelectQuery};
use ruprizzle_testkit::both_dbs;

#[derive(Debug, Clone, PartialEq, Default, ::ruprizzle::sqlx::FromRow)]
struct Employee {
    id: i64,
    name: String,
    role: String,
    age: i64,
    salary: f64,
}

#[cfg(feature = "postgres-tokio-postgres")]
impl ruprizzle::tokio_postgres::FromTokioPostgresRow for Employee {
    fn from_tokio_postgres_row(
        row: &ruprizzle::tokio_postgres::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.try_get::<usize, i64>(0).map_err(ruprizzle::Error::TokioPostgres)?,
            name: row.try_get::<usize, String>(1).map_err(ruprizzle::Error::TokioPostgres)?,
            role: row.try_get::<usize, String>(2).map_err(ruprizzle::Error::TokioPostgres)?,
            age: row.try_get::<usize, i64>(3).map_err(ruprizzle::Error::TokioPostgres)?,
            salary: row.try_get::<usize, f64>(4).map_err(ruprizzle::Error::TokioPostgres)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Employee {
    fn from_rusqlite_row(
        row: &ruprizzle::rusqlite::RusqliteRow,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            name: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            role: ::ruprizzle::rusqlite::get::<String>(row, 2)?,
            age: ::ruprizzle::rusqlite::get::<i64>(row, 3)?,
            salary: ::ruprizzle::rusqlite::get::<f64>(row, 4)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Employee {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            name: row.get::<String>(1)?,
            role: row.get::<String>(2)?,
            age: row.get::<i64>(3)?,
            salary: row.get::<f64>(4)?,
        })
    }
}

impl Model for Employee {
    const TABLE: &'static str = "employees";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name", "role", "age", "salary"];
}

const ID: Column<Employee, i64> = Column::new("employees", "id");
const NAME: Column<Employee, String> = Column::new("employees", "name");
const ROLE: Column<Employee, String> = Column::new("employees", "role");
const AGE: Column<Employee, i64> = Column::new("employees", "age");
const SALARY: Column<Employee, f64> = Column::new("employees", "salary");

const SETUP_SQL: &str = r#"
CREATE TABLE employees (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    age BIGINT NOT NULL,
    salary DOUBLE PRECISION NOT NULL
);
INSERT INTO employees (id, name, role, age, salary) VALUES
(1, 'Alice', 'Engineer', 30, 100000),
(2, 'Bob', 'Engineer', 35, 120000),
(3, 'Carol', 'Manager', 45, 150000),
(4, 'Dave', 'Manager', 50, 180000);
"#;

both_dbs! {
    setup = SETUP_SQL;
    #[allow(clippy::float_cmp)]
    async fn sum_and_count(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .aggregate((SALARY.sum(), ID.count()));
        let compiled = q.to_sql();
        insta::assert_snapshot!(
            format!("sum_count_{}", db.backend().as_str()),
            compiled.sql.as_ref(),
        );
        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![(Some(550000.0), 4)]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    #[allow(clippy::float_cmp)]
    async fn avg_min_max(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .aggregate((AGE.avg(), AGE.min(), AGE.max()));
        let compiled = q.to_sql();
        insta::assert_snapshot!(
            format!("avg_min_max_{}", db.backend().as_str()),
            compiled.sql.as_ref(),
        );
        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![(Some(40.0), Some(30), Some(50))]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    async fn count_distinct_name_and_role(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .aggregate((NAME.count_distinct(), ROLE.count_distinct()));
        let compiled = q.to_sql();
        insta::assert_snapshot!(
            format!("count_distinct_{}", db.backend().as_str()),
            compiled.sql.as_ref(),
        );
        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![(4, 2)]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    #[allow(clippy::float_cmp)]
    async fn group_by_role_with_sum(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .group_by(ROLE)
            .aggregate((SALARY.sum(), ID.count()))
            .order_by(ROLE.asc());
        let compiled = q.to_sql();
        insta::assert_snapshot!(
            format!("group_by_role_{}", db.backend().as_str()),
            compiled.sql.as_ref(),
        );
        let rows = q.fetch_all().await?;
        assert_eq!(
            rows,
            vec![(Some(220000.0), 2), (Some(330000.0), 2)],
        );
    }
}

both_dbs! {
    setup = SETUP_SQL;
    #[allow(clippy::float_cmp)]
    async fn having_role(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .group_by(ROLE)
            .having(ROLE.eq("Manager"))
            .aggregate((SALARY.sum(),));
        let compiled = q.to_sql();
        insta::assert_snapshot!(
            format!("having_role_{}", db.backend().as_str()),
            compiled.sql.as_ref(),
        );
        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![(Some(330000.0),)]);
    }
}

both_dbs! {
    setup = SETUP_SQL;
    #[allow(clippy::float_cmp)]
    async fn aggregate_with_where_and_group_by(db: TestDb) {
        let q = SelectQuery::<Employee>::new(db.pool())
            .filter(AGE.gt(35))
            .group_by(ROLE)
            .aggregate((SALARY.sum(), ID.count()))
            .order_by(ROLE.asc());
        let compiled = q.to_sql();
        insta::assert_snapshot!(
            format!("where_group_{}", db.backend().as_str()),
            compiled.sql.as_ref(),
        );
        let rows = q.fetch_all().await?;
        assert_eq!(rows, vec![(Some(330000.0), 2)]);
    }
}
