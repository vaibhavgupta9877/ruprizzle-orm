//! Runtime query-builder edge cases against a local SQLite file.
//!
//! These are not quick unit tests of SQL text; they hit a real database and
//! verify that the query builder's type-system guarantees hold end to end.

use futures_util::StreamExt;
use ruprizzle::{
    Column, DeleteQuery, InsertQuery, Model, SelectQuery, UpdateQuery,
};
use ruprizzle_deep_tests::fresh_pool;

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
struct Item {
    id: i64,
    handle: String,
    age: i64,
    active: i64,
    note: Option<String>,
}

impl Model for Item {
    const TABLE: &'static str = "items";
}

const ID: Column<Item, i64> = Column::new("items", "id");
const HANDLE: Column<Item, String> = Column::new("items", "handle");
const AGE: Column<Item, i64> = Column::new("items", "age");
const ACTIVE: Column<Item, i64> = Column::new("items", "active");
const NOTE: Column<Item, Option<String>> = Column::new("items", "note");

async fn seed(pool: &ruprizzle::Pool) {
    sqlx::query(
        "CREATE TABLE items (
            id INTEGER PRIMARY KEY,
            handle TEXT NOT NULL,
            age INTEGER NOT NULL,
            active INTEGER NOT NULL,
            note TEXT
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    let rows: [(i64, &'static str, i64, i64, Option<&'static str>); 5] = [
        (1, "alpha", 10, 1, None),
        (2, "beta", 20, 0, Some("first")),
        (3, "gamma", 30, 1, None),
        (4, "delta", 40, 1, Some("second")),
        (5, "epsilon", 25, 0, None),
    ];

    for (id, handle, age, active, note) in rows {
        let mut q = InsertQuery::<Item>::new(pool)
            .set(ID, id)
            .set(HANDLE, handle)
            .set(AGE, age)
            .set(ACTIVE, active);
        q = match note {
            Some(n) => q.set(NOTE, Some(n.to_string())),
            None => q.set(NOTE, None::<String>),
        };
        q.exec().await.unwrap();
    }
}

#[tokio::test]
async fn comparison_and_between_filters() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(AGE.gt(15))
        .filter(AGE.lt(35))
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, 2);
    assert_eq!(rows[1].id, 3);
    assert_eq!(rows[2].id, 5);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(AGE.between(20, 30))
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, 2);
    assert_eq!(rows[1].id, 3);
    assert_eq!(rows[2].id, 5);
}

#[tokio::test]
async fn in_set_and_not_in_set() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(HANDLE.in_set(["alpha", "gamma", "epsilon"]))
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 3, 5]);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(HANDLE.not_in_set(["alpha", "gamma"]))
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 4, 5]);
}

#[tokio::test]
async fn string_matchers_and_null_filters() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(HANDLE.starts_with("e"))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 5);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(HANDLE.ends_with("ma"))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 3);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(HANDLE.contains("lt"))
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 4);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(NOTE.is_null())
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 3, 5]);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(NOTE.is_not_null())
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 4]);
}

#[tokio::test]
async fn and_or_all_any_combinators() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(ACTIVE.eq(1).and(AGE.gt(15)))
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![3, 4]);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(AGE.eq(10).or(AGE.eq(40)))
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 4]);

    let f = ruprizzle::all([
        ACTIVE.eq(1),
        AGE.gt(15),
        AGE.lt(35),
    ]);
    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(f)
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![3]);

    let f = ruprizzle::any([AGE.eq(10), AGE.eq(30), AGE.eq(50)]);
    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .filter(f)
        .order_by(ID.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 3]);
}

#[tokio::test]
async fn projection_distinct_count_exists_stream() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let names: Vec<(String,)> = SelectQuery::<Item>::new(&pool)
        .columns((HANDLE,))
        .order_by(HANDLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(names.len(), 5);
    assert_eq!(names[0].0, "alpha");

    let pairs: Vec<(String, i64)> = SelectQuery::<Item>::new(&pool)
        .columns((HANDLE, AGE))
        .order_by(HANDLE.asc())
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(pairs[0], ("alpha".to_string(), 10));

    let active_values: Vec<(i64,)> = SelectQuery::<Item>::new(&pool)
        .columns((ACTIVE,))
        .distinct()
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(active_values.len(), 2);

    let count = SelectQuery::<Item>::new(&pool)
        .filter(ACTIVE.eq(1))
        .count()
        .await
        .unwrap();
    assert_eq!(count, 3);

    let exists = SelectQuery::<Item>::new(&pool)
        .filter(HANDLE.eq("missing"))
        .exists()
        .await
        .unwrap();
    assert!(!exists);

    let exists = SelectQuery::<Item>::new(&pool)
        .filter(AGE.between(20, 30))
        .exists()
        .await
        .unwrap();
    assert!(exists);

    let mut stream = SelectQuery::<Item>::new(&pool)
        .filter(ACTIVE.eq(1))
        .order_by(ID.asc())
        .stream();
    let mut ids = Vec::new();
    while let Some(row) = stream.next().await {
        ids.push(row.unwrap().id);
    }
    assert_eq!(ids, vec![1, 3, 4]);
}

#[tokio::test]
async fn pagination_and_cursors() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let page: ruprizzle::Page<Item> = SelectQuery::<Item>::new(&pool)
        .order_by(AGE.asc())
        .page(2)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page.has_next);
    assert_eq!(page.items[0].age, 10);
    assert_eq!(page.items[1].age, 20);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .order_by(AGE.asc())
        .offset(2)
        .limit(2)
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].age, 25);
    assert_eq!(rows[1].age, 30);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .after(ID, 2, 2)
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, 3);
    assert_eq!(rows[1].id, 4);

    let rows: Vec<Item> = SelectQuery::<Item>::new(&pool)
        .before(ID, 5, 2)
        .fetch_all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, 4);
    assert_eq!(rows[1].id, 3);
}

#[tokio::test]
async fn update_and_delete_guards() {
    let (pool, _tmp) = fresh_pool().await;
    seed(&pool).await;

    let affected = UpdateQuery::<Item>::new(&pool)
        .filter(ID.eq(1))
        .set(HANDLE, "alpha-updated")
        .set_null(NOTE)
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let row = SelectQuery::<Item>::new(&pool)
        .filter(ID.eq(1))
        .fetch_one()
        .await
        .unwrap();
    assert_eq!(row.handle, "alpha-updated");
    assert!(row.note.is_none());

    let err = UpdateQuery::<Item>::new(&pool)
        .set(HANDLE, "oops")
        .exec()
        .await;
    assert!(err.is_err());

    let affected = DeleteQuery::<Item>::new(&pool)
        .filter(ID.eq(5))
        .exec()
        .await
        .unwrap();
    assert_eq!(affected, 1);
    assert_eq!(SelectQuery::<Item>::new(&pool).count().await.unwrap(), 4);

    let affected = DeleteQuery::<Item>::new(&pool).all_rows().exec().await.unwrap();
    assert_eq!(affected, 4);
    assert_eq!(SelectQuery::<Item>::new(&pool).count().await.unwrap(), 0);
}
