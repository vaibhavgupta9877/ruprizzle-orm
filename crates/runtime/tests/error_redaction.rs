//! User data must not reach logs through an error's Display.

use ruprizzle::Error;

#[test]
fn unique_violation_display_omits_the_value() {
    let error = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: Some("alice@example.com".to_owned()),
    };
    let rendered = error.to_string();
    assert_eq!(rendered, "unique constraint violated on `users.email`");
    assert!(!rendered.contains("alice@example.com"));
}

#[test]
fn conflicting_value_is_available_explicitly() {
    let error = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: Some("alice@example.com".to_owned()),
    };
    assert_eq!(error.conflicting_value(), Some("alice@example.com"));
}

#[test]
fn conflicting_value_is_none_when_not_captured_or_not_unique() {
    let without_value = Error::UniqueViolation {
        table: "users".to_owned(),
        columns: "email".to_owned(),
        value: None,
    };
    assert_eq!(without_value.conflicting_value(), None);
    assert_eq!(Error::Deadlock.conflicting_value(), None);
}
