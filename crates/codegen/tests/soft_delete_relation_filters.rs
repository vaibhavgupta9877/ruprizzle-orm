//! Relation filters ignore soft-deleted children (K4).
//!
//! `user::posts_some(f)` used to compile to `EXISTS (SELECT 1 FROM posts WHERE
//! posts.author_id = users.id AND f)`, so "has a post" matched a user whose only
//! post was soft-deleted. The generated filter now adds the child's
//! `deleted_at IS NULL`, and only when the child is soft-deletable.

use ruprizzle_codegen::generate_all;
use ruprizzle_parser::parse;

const SCHEMA: &str = r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model User {
  id       Int       @id
  posts    Post[]
  sessions Session[]
}

model Post {
  id        Int       @id
  authorId  Int       @map("author_id")
  author    User      @relation(fields: [authorId], references: [id])
  deletedAt DateTime? @deletedAt @map("deleted_at")
}

model Session {
  id     Int  @id
  userId Int  @map("user_id")
  user   User @relation(fields: [userId], references: [id])
}
"#;

/// The body of the generated `pub fn <name>(…) { … }`, whitespace-normalised.
fn function_body(files: &std::collections::BTreeMap<String, String>, name: &str) -> String {
    let needle = format!("pub fn {name}(");
    let (_, content) = files
        .iter()
        .find(|(_, c)| c.contains(&needle))
        .unwrap_or_else(|| panic!("no generated file defines {name}"));
    let start = content.find(&needle).unwrap();
    let rest = &content[start..];
    let end = rest[needle.len()..]
        .find("pub fn ")
        .map_or(rest.len(), |i| i + needle.len());
    rest[..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn relation_filters_on_a_soft_deletable_child_skip_deleted_rows() {
    let schema = parse("schema.ruprizzle", SCHEMA).expect("schema parses");
    let files = generate_all(&schema);

    for name in ["posts_some", "posts_none", "posts_every"] {
        let body = function_body(&files, name);
        assert!(
            body.contains("FilterNode::Null") && body.contains("\"deleted_at\""),
            "{name} must add the child's deleted_at IS NULL:\n{body}"
        );
    }

    // `every` negates the user's filter, not the liveness check: it ranges over
    // live children only.
    let every = function_body(&files, "posts_every");
    assert!(every.contains("(!f)"), "{every}");
    assert!(!every.contains("!(f"), "{every}");

    // A child without @deletedAt gets no extra predicate.
    for name in ["sessions_some", "sessions_none", "sessions_every"] {
        let body = function_body(&files, name);
        assert!(!body.contains("FilterNode::Null"), "{name}:\n{body}");
    }
}
