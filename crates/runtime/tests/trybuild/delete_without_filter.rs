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
    fn from_rusqlite_row(
        row: &mut ruprizzle::rusqlite::Row,
    ) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ruprizzle::rusqlite::FromValue::from_value(row.0.remove(0))?,
        })
    }
}

fn delete_all(db: &Pool) {
    let _ = DeleteQuery::<Task>::new(db).exec();
}

fn main() {}
