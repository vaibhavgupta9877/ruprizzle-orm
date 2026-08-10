//! Shared fixtures for the cross-crate integration suite.
//!
//! The tests themselves live in `tests/`. This library exists to hold schema
//! fixtures that more than one of them needs, so they do not drift apart.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// Minimal DDL used by the P0 harness smoke tests.
///
/// Deliberately written in the intersection of Postgres and `SQLite` syntax, since
/// no dialect layer exists yet to translate. From P2 onward the harness takes a
/// `schema.ruprizzle` and generates this instead.
pub const SMOKE_DDL: &str = "CREATE TABLE widget (
    id    INTEGER PRIMARY KEY,
    name  TEXT    NOT NULL,
    price INTEGER NOT NULL
);
CREATE TABLE widget_part (
    id        INTEGER PRIMARY KEY,
    widget_id INTEGER NOT NULL REFERENCES widget(id) ON DELETE CASCADE,
    label     TEXT    NOT NULL
);
";
