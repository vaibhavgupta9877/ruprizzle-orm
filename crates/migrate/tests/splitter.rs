//! Tests for the migration statement splitter.

use ruprizzle_migrate::runner::split_statements;

#[test]
fn preserves_non_ascii_text() {
    let out = split_statements("INSERT INTO t (name) VALUES ('café');");
    assert_eq!(out, vec!["INSERT INTO t (name) VALUES ('café')"]);
}

#[test]
fn preserves_multibyte_outside_literals() {
    let out = split_statements("COMMENT ON TABLE t IS 'naïve — 日本語';");
    assert_eq!(out, vec!["COMMENT ON TABLE t IS 'naïve — 日本語'"]);
}

#[test]
fn keeps_dollar_quoted_body_intact() {
    let sql =
        "CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql;";
    let out = split_statements(sql);
    assert_eq!(out.len(), 1, "got {out:?}");
    assert!(out[0].contains("BEGIN RETURN NEW; END;"));
}

#[test]
fn keeps_tagged_dollar_quote_intact() {
    let sql = "CREATE FUNCTION f() RETURNS text AS $body$ SELECT 'a;b'; $body$ LANGUAGE sql;";
    let out = split_statements(sql);
    assert_eq!(out.len(), 1, "got {out:?}");
}

#[test]
fn comment_inside_dollar_quote_is_not_stripped() {
    let sql = "CREATE FUNCTION f() RETURNS int AS $$ -- keep me\n SELECT 1; $$ LANGUAGE sql;";
    let out = split_statements(sql);
    assert_eq!(out.len(), 1, "got {out:?}");
    assert!(out[0].contains("-- keep me"));
}

#[test]
fn bind_placeholder_is_not_a_dollar_quote() {
    let out = split_statements("SELECT * FROM t WHERE a = $1 AND b = $2; SELECT 1;");
    assert_eq!(
        out,
        vec!["SELECT * FROM t WHERE a = $1 AND b = $2", "SELECT 1"]
    );
}

#[test]
fn still_splits_plain_statements_and_strips_comments() {
    let sql = "CREATE TABLE a (id int); -- note\n/* block */ CREATE TABLE b (id int);";
    let out = split_statements(sql);
    assert_eq!(out.len(), 2, "got {out:?}");
    assert!(!out[1].contains("note"));
    assert!(!out[1].contains("block"));
}

#[test]
fn semicolon_inside_string_literal_does_not_split() {
    let out = split_statements("INSERT INTO t (s) VALUES ('a;b');");
    assert_eq!(out, vec!["INSERT INTO t (s) VALUES ('a;b')"]);
}
