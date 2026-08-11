//! Probe how the `Any` driver binds `Value::Null` in the middle of a parameter list.

use ruprizzle::Value;
use ruprizzle_deep_tests::fresh_pool;

#[tokio::test]
async fn value_null_in_middle_does_not_shift() {
    let (pool, _tmp) = fresh_pool().await;

    let row: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT ? AS a, ? AS b, ? AS c",
    )
    .bind(&Value::Str("hello".into()))
    .bind(&Value::Null)
    .bind(&Value::I64(42))
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "hello");
    assert_eq!(row.1, None);
    assert_eq!(row.2, 42);
}

#[tokio::test]
async fn option_none_in_middle_does_not_shift() {
    let (pool, _tmp) = fresh_pool().await;

    let row: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT ? AS a, ? AS b, ? AS c",
    )
    .bind("hello".to_string())
    .bind(None::<String>)
    .bind(42i64)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "hello");
    assert_eq!(row.1, None);
    assert_eq!(row.2, 42);
}
