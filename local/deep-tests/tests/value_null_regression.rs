//! Probe how the runtime binds `Value::Null` in the middle of a parameter list.

use std::borrow::Cow;

use ruprizzle::{Executor, Value, decode_rows};
use ruprizzle_deep_tests::fresh_pool;

#[tokio::test]
async fn value_null_in_middle_does_not_shift() {
    let (pool, _tmp) = fresh_pool().await;

    let batch = pool
        .fetch_all_raw(
            Cow::Borrowed("SELECT ? AS a, ? AS b, ? AS c"),
            vec![
                Value::Str("hello".into()),
                Value::Null,
                Value::I64(42),
            ],
        )
        .await
        .unwrap();
    let mut rows: Vec<(String, Option<String>, i64)> = decode_rows(batch).unwrap();
    assert_eq!(rows.len(), 1);
    let row = rows.pop().unwrap();
    assert_eq!(row.0, "hello");
    assert_eq!(row.1, None);
    assert_eq!(row.2, 42);
}

#[tokio::test]
async fn option_none_in_middle_does_not_shift() {
    let (pool, _tmp) = fresh_pool().await;

    let batch = pool
        .fetch_all_raw(
            Cow::Borrowed("SELECT ? AS a, ? AS b, ? AS c"),
            vec![
                Value::Str("hello".into()),
                Value::Null,
                Value::I64(42),
            ],
        )
        .await
        .unwrap();
    let mut rows: Vec<(String, Option<String>, i64)> = decode_rows(batch).unwrap();
    assert_eq!(rows.len(), 1);
    let row = rows.pop().unwrap();
    assert_eq!(row.0, "hello");
    assert_eq!(row.1, None);
    assert_eq!(row.2, 42);
}
