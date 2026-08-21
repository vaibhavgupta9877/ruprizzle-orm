//! Integration tests for Implicit Many-to-Many join tables (v1.3.0).

use ruprizzle_core::ir::RelationKind;
use ruprizzle_parser::parse;

#[tokio::test]
async fn test_implicit_many_to_many_schema_lowering() {
    let schema_text = r#"
        datasource db {
            provider = "sqlite"
            url      = "file:test.db"
        }

        model Post {
            id    String @id @default(uuid4())
            title String
            tags  Tag[]
        }

        model Tag {
            id    String @id @default(uuid4())
            name  String @unique
            posts Post[]
        }
    "#;

    let schema = parse("schema.prisma", schema_text).expect("schema should parse and lower");

    // 1. Check that the implicit join model `_PostToTag` was automatically synthesized
    let join_model = schema
        .model("_PostToTag")
        .expect("synthetic join model _PostToTag must exist in schema.models");

    assert_eq!(join_model.table, "_PostToTag");
    let pk_cols: Vec<&str> = join_model
        .primary_key
        .fields
        .iter()
        .map(|f| f.as_str())
        .collect();
    assert_eq!(pk_cols, vec!["a", "b"]);
    assert!(join_model.field("a").is_some());
    assert!(join_model.field("b").is_some());
    assert!(join_model.field("a_rel").is_some());
    assert!(join_model.field("b_rel").is_some());

    // 2. Check that Post.tags and Tag.posts have `through` wired to `_PostToTag`
    let post_model = schema.model("Post").unwrap();
    let post_tags = post_model.field("tags").unwrap();
    let post_tags_rel = post_tags.relation().unwrap();
    assert_eq!(
        post_tags_rel.through.as_ref().map(|m| m.as_str()),
        Some("_PostToTag")
    );

    let tag_model = schema.model("Tag").unwrap();
    let tag_posts = tag_model.field("posts").unwrap();
    let tag_posts_rel = tag_posts.relation().unwrap();
    assert_eq!(
        tag_posts_rel.through.as_ref().map(|m| m.as_str()),
        Some("_PostToTag")
    );

    // 3. Check resolved relations contain ManyToMany
    let m2m_relations: Vec<_> = schema
        .relations
        .iter()
        .filter(|r| r.kind == RelationKind::ManyToMany)
        .collect();
    assert_eq!(m2m_relations.len(), 2);
    assert_eq!(
        m2m_relations[0].join_model.as_ref().map(|m| m.as_str()),
        Some("_PostToTag")
    );
    assert_eq!(
        m2m_relations[1].join_model.as_ref().map(|m| m.as_str()),
        Some("_PostToTag")
    );
}

#[tokio::test]
async fn test_implicit_m2m_migration_diff_generates_create_table() {
    let empty_schema_text = r#"
        datasource db {
            provider = "sqlite"
            url      = "file:test.db"
        }
    "#;

    let schema_text = r#"
        datasource db {
            provider = "sqlite"
            url      = "file:test.db"
        }

        model Post {
            id    String @id @default(uuid4())
            title String
            tags  Tag[]
        }

        model Tag {
            id    String @id @default(uuid4())
            name  String @unique
            posts Post[]
        }
    "#;

    let prev = parse("schema.prisma", empty_schema_text).unwrap();
    let next = parse("schema.prisma", schema_text).unwrap();

    let changes = ruprizzle_migrate::diff(&prev, &next);

    // Verify CreateModel for Post, Tag, and synthesized _PostToTag
    let created_models: Vec<String> = changes
        .iter()
        .filter_map(|c| match c {
            ruprizzle_migrate::change::Change::CreateModel(m) => Some(m.name.to_string()),
            _ => None,
        })
        .collect();

    assert!(created_models.contains(&"Post".to_string()));
    assert!(created_models.contains(&"Tag".to_string()));
    assert!(created_models.contains(&"_PostToTag".to_string()));
}
