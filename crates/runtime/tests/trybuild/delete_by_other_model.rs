use ruprizzle::{Column, DeleteQuery, Model, Pool};

#[derive(sqlx::FromRow)]
struct Task {
    id: i64,
}

impl Model for Task {
    const TABLE: &'static str = "tasks";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Task {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Task {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct User {
    id: i64,
}

impl Model for User {
    const TABLE: &'static str = "users";
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
        })
    }
}

const USER_ID: Column<User, i64> = Column::new("users", "id");

fn delete_by_user(db: &Pool) {
    let _ = DeleteQuery::<Task>::new(db).filter(USER_ID.eq(1)).exec();
}

fn main() {}
