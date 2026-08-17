//! P1-01/P1-02 acceptance: every schema under `examples/` parses and lowers to
//! the IR we expect.
//!
//! The IR is snapshotted rather than asserted field-by-field. It is large, and a
//! hand-written expectation for each of these would be both unreadable and
//! incomplete; `cargo insta review` turns any change to lowering into a diff a
//! human can actually read.

use ruprizzle_core::ir::{FieldKind, ReferentialAction, RelationKind, ScalarType};
use ruprizzle_parser::parse_with_warnings;

const EXAMPLES: &[&str] = &["blog", "ecommerce", "saas-tenant", "minimal"];

fn read(example: &str) -> (String, String) {
    let path = format!(
        "{}/../../examples/{example}/schema.ruprizzle",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read example `{example}` at {path}: {e}"));
    (format!("{example}/schema.ruprizzle"), source)
}

fn read_fixture(name: &str) -> (String, String) {
    let path = format!(
        "{}/tests/fixtures/{name}/schema.ruprizzle",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture `{name}` at {path}: {e}"));
    (format!("{name}/schema.ruprizzle"), source)
}

#[test]
fn every_example_lowers_to_the_expected_ir() {
    for example in EXAMPLES {
        let (name, source) = read(example);
        let (schema, warnings) = match parse_with_warnings(&name, &source) {
            Ok(ok) => ok,
            Err(e) => panic!("{example} failed to parse:\n{:?}", miette::Report::new(*e)),
        };
        assert!(
            warnings.is_empty(),
            "{example} should be warning-free, got {warnings:?}"
        );
        insta::assert_json_snapshot!(*example, schema);
    }
}

#[test]
fn naming_conventions_are_applied_once_during_lowering() {
    let (name, source) = read("blog");
    let schema = ruprizzle_parser::parse(&name, &source).expect("blog is valid");

    let user = schema.model("User").expect("User exists");
    assert_eq!(user.table, "users", "`@@map` wins over the convention");
    assert_eq!(
        user.field("createdAt").expect("field exists").column,
        "created_at"
    );
    assert_eq!(user.field("email").expect("field exists").column, "email");
    assert_eq!(
        schema.enum_def("Role").expect("Role exists").db_name,
        "role"
    );

    // No `@@map`, so the convention applies: PascalCase → snake_case → plural.
    let (name, source) = read("ecommerce");
    let schema = ruprizzle_parser::parse(&name, &source).expect("ecommerce is valid");
    assert_eq!(
        schema.model("OrderItem").expect("exists").table,
        "order_items"
    );
    assert_eq!(schema.model("Customer").expect("exists").table, "customers");
}

#[test]
fn relations_are_canonical_and_both_sides_agree() {
    let (name, source) = read("blog");
    let schema = ruprizzle_parser::parse(&name, &source).expect("blog is valid");

    assert_eq!(schema.relations.len(), 1);

    let post_author = schema
        .model("Post")
        .expect("Post exists")
        .field("author")
        .expect("author exists");
    let user_posts = schema
        .model("User")
        .expect("User exists")
        .field("posts")
        .expect("posts exists");

    let a = schema.relation(post_author.relation().expect("is a relation"));
    let b = schema.relation(user_posts.relation().expect("is a relation"));
    assert!(a.is_some() && a == b, "both sides must reach one entry");

    let rel = a.expect("resolved");
    assert_eq!(rel.kind, RelationKind::ManyToOne);
    assert_eq!(rel.owner.as_str(), "Post");
    assert_eq!(rel.owner_cols, vec!["author_id".to_owned()]);
    assert_eq!(rel.target_cols, vec!["id".to_owned()]);
    assert_eq!(rel.on_delete, ReferentialAction::Cascade);
    assert_eq!(rel.constraint_name, "posts_author_id_fkey");
    assert!(!user_posts.has_column(), "a list side owns no column");
}

#[test]
fn composite_keys_and_named_relations_survive_lowering() {
    let (name, source) = read("ecommerce");
    let schema = ruprizzle_parser::parse(&name, &source).expect("ecommerce is valid");
    let item = schema.model("OrderItem").expect("OrderItem exists");
    assert!(item.primary_key.is_composite());
    assert_eq!(item.primary_key.fields.len(), 2);

    let (name, source) = read_fixture("social");
    let schema = ruprizzle_parser::parse(&name, &source).expect("social is valid");
    // follower, followee, threadAuthor, threadParent — the self-relation included.
    assert_eq!(schema.relations.len(), 4);
    let names: Vec<&str> = schema.relations.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"threadParent"), "got {names:?}");

    let parent = schema
        .relation(
            schema
                .model("Thread")
                .expect("Thread exists")
                .field("parent")
                .expect("parent exists")
                .relation()
                .expect("is a relation"),
        )
        .expect("resolved");
    assert!(parent.optional);
    // An optional relation defaults to SET NULL: the row can simply lose its
    // parent, which a required one cannot.
    assert_eq!(parent.on_delete, ReferentialAction::SetNull);
}

#[test]
fn scalar_types_and_docs_reach_the_ir() {
    let (name, source) = read("ecommerce");
    let schema = ruprizzle_parser::parse(&name, &source).expect("ecommerce is valid");
    let price = schema
        .model("Product")
        .expect("Product exists")
        .field("price")
        .expect("price exists");
    assert_eq!(price.kind, FieldKind::Scalar(ScalarType::Decimal));
    assert!(
        price.docs.is_some(),
        "`///` must reach the IR so codegen can emit rustdoc"
    );

    let native = schema
        .model("Product")
        .expect("Product exists")
        .field("name")
        .expect("name exists")
        .attrs
        .native_type
        .as_ref()
        .expect("@db.VarChar(120)");
    assert_eq!(native.name, "VarChar");
    assert_eq!(native.args, vec!["120".to_owned()]);
}

#[test]
fn the_fingerprint_is_stable_across_runs() {
    let (name, source) = read("blog");
    let a = ruprizzle_parser::parse(&name, &source).expect("valid");
    let b = ruprizzle_parser::parse(&name, &source).expect("valid");
    assert_eq!(a.fingerprint(), b.fingerprint());
}

#[test]
fn many_to_many_through_is_resolved() {
    let source = r#"
        datasource db {
            provider = "postgres"
            url      = env("DATABASE_URL")
        }

        generator client {
            output = "src/db"
        }

        model Post {
            id   Int   @id
            tags Tag[] @relation(through: PostTag)
        }

        model Tag {
            id    Int     @id
            posts Post[]  @relation(through: PostTag)
        }

        model PostTag {
            postId Int
            tagId  Int
            post   Post @relation(fields: [postId], references: [id])
            tag    Tag  @relation(fields: [tagId], references: [id])
            @@id([postId, tagId])
            @@map("post_tags")
        }
    "#;

    let schema = ruprizzle_parser::parse("m2m.ruprizzle", source).expect("valid m2m");

    let m2m = schema
        .relations
        .iter()
        .find(|r| r.kind == RelationKind::ManyToMany)
        .expect("m2m relation exists");
    assert_eq!(m2m.owner.as_str(), "Post");
    assert_eq!(m2m.target.as_str(), "Tag");
    assert_eq!(m2m.join_model.as_ref().unwrap().as_str(), "PostTag");
    assert_eq!(m2m.join_owner_field.as_ref().unwrap().as_str(), "post");
    assert_eq!(m2m.join_target_field.as_ref().unwrap().as_str(), "tag");
    assert_eq!(m2m.owner_cols, vec!["post_id".to_owned()]);
    assert_eq!(m2m.target_cols, vec!["tag_id".to_owned()]);

    // Each list side resolves to its own oriented many-to-many relation.
    let m2m_relations: Vec<_> = schema
        .relations
        .iter()
        .filter(|r| r.kind == RelationKind::ManyToMany)
        .collect();
    assert_eq!(m2m_relations.len(), 2);

    let post_tags = schema.model("Post").unwrap().field("tags").unwrap();
    let tag_posts = schema.model("Tag").unwrap().field("posts").unwrap();
    assert_eq!(post_tags.relation().unwrap().resolved, Some(2));
    assert_eq!(tag_posts.relation().unwrap().resolved, Some(3));

    // The join model holds the two real FK relations.
    let post_tag = schema.model("PostTag").unwrap();
    let post = post_tag.field("post").unwrap().relation().unwrap();
    let tag = post_tag.field("tag").unwrap().relation().unwrap();
    assert!(post.resolved.is_some());
    assert!(tag.resolved.is_some());
}
