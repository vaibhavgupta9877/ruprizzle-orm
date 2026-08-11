//! Tests for the injection-safe `raw!` macro.

use ruprizzle::raw;

#[test]
fn raw_binds_rather_than_interpolates() {
    let email = "'; DROP TABLE users; --";
    let fragment = raw!("email = {}", email);
    assert_eq!(fragment.sql(), "email = $1");
    assert_eq!(fragment.binds().len(), 1);
    assert!(
        !fragment.sql().contains("DROP"),
        "raw! interpolated user data into SQL"
    );
}

#[test]
fn raw_multiple_placeholders() {
    let fragment = raw!("{} = {} AND id > {}", "email", "a@b.c", 42_i64);
    assert_eq!(fragment.sql(), "$1 = $2 AND id > $3");
    assert_eq!(fragment.binds().len(), 3);
    assert_eq!(
        fragment.binds(),
        &[
            ruprizzle::Value::Str(std::sync::Arc::from("email")),
            ruprizzle::Value::Str(std::sync::Arc::from("a@b.c")),
            ruprizzle::Value::I64(42),
        ]
    );
}

#[test]
fn raw_fragment_sql_for_two_binds() {
    let fragment = raw!("x = {} AND y = {}", 1_i64, 2_i64);
    assert_eq!(fragment.sql(), "x = $1 AND y = $2");
    assert_eq!(
        fragment.binds(),
        &[ruprizzle::Value::I64(1), ruprizzle::Value::I64(2)]
    );
}
