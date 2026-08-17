//! P0-02 acceptance: a hand-built [`Schema`] survives a JSON round trip.
//!
//! This matters beyond the obvious: the migration snapshot format *is* the
//! serialized IR (ADR-007), so a type that does not round-trip faithfully is a
//! migration engine that silently forgets part of the schema.

use indexmap::IndexMap;
use ruprizzle_core::ir::{
    Datasource, DatasourceUrl, DefaultFn, DefaultValue, EnumDef, EnumVariant, Field, FieldAttrs,
    FieldKind, Generator, IR_VERSION, IndexDef, IndexTarget, Literal, Model, PrimaryKey, Provider,
    ReferentialAction, RelationKind, RelationRef, ResolvedRelation, ScalarType, Schema, SortOrder,
};
use ruprizzle_core::names::{EnumName, FieldName, ModelName};
use ruprizzle_core::span::Span;

fn field(name: &str, column: &str, kind: FieldKind) -> Field {
    Field {
        name: FieldName::new(name),
        column: column.to_owned(),
        kind,
        optional: false,
        default: None,
        attrs: FieldAttrs::default(),
        generated: None,
        docs: None,
        span: Span::new(0, 1),
    }
}

/// The `blog` example, built by hand: two models, one enum, one relation.
fn blog_schema() -> Schema {
    let mut role_variants = IndexMap::new();
    role_variants.insert(
        "USER".to_owned(),
        EnumVariant {
            name: "USER".to_owned(),
            db_name: "USER".to_owned(),
            docs: None,
            span: Span::new(10, 14),
        },
    );
    role_variants.insert(
        "ADMIN".to_owned(),
        EnumVariant {
            name: "ADMIN".to_owned(),
            db_name: "ADMIN".to_owned(),
            docs: Some("Full access.".to_owned()),
            span: Span::new(15, 20),
        },
    );

    let mut enums = IndexMap::new();
    enums.insert(
        EnumName::new("Role"),
        EnumDef {
            name: EnumName::new("Role"),
            db_name: "role".to_owned(),
            variants: role_variants,
            docs: None,
            span: Span::new(5, 25),
        },
    );

    // --- User ---
    let mut user_fields = IndexMap::new();
    let mut id = field("id", "id", FieldKind::Scalar(ScalarType::Uuid));
    id.attrs.is_id = true;
    id.default = Some(DefaultValue::Function(DefaultFn::Uuid7));
    user_fields.insert(FieldName::new("id"), id);

    let mut email = field("email", "email", FieldKind::Scalar(ScalarType::String));
    email.attrs.is_unique = true;
    user_fields.insert(FieldName::new("email"), email);

    let mut name = field("name", "name", FieldKind::Scalar(ScalarType::String));
    name.optional = true;
    user_fields.insert(FieldName::new("name"), name);

    let mut role = field("role", "role", FieldKind::Enum(EnumName::new("Role")));
    role.default = Some(DefaultValue::Literal(Literal::EnumVariant(
        "USER".to_owned(),
    )));
    user_fields.insert(FieldName::new("role"), role);

    user_fields.insert(
        FieldName::new("posts"),
        field(
            "posts",
            "",
            FieldKind::List(Box::new(FieldKind::Relation(RelationRef {
                target: ModelName::new("Post"),
                name: None,
                through: None,
                fields: vec![],
                references: vec![],
                on_delete: None,
                on_update: None,
                resolved: Some(0),
                span: Span::new(60, 70),
            }))),
        ),
    );

    // --- Post ---
    let mut post_fields = IndexMap::new();
    let mut post_id = field("id", "id", FieldKind::Scalar(ScalarType::Uuid));
    post_id.attrs.is_id = true;
    post_fields.insert(FieldName::new("id"), post_id);

    let mut title = field("title", "title", FieldKind::Scalar(ScalarType::String));
    title.docs = Some("Headline.".to_owned());
    post_fields.insert(FieldName::new("title"), title);

    post_fields.insert(
        FieldName::new("authorId"),
        field("authorId", "author_id", FieldKind::Scalar(ScalarType::Uuid)),
    );
    post_fields.insert(
        FieldName::new("author"),
        field(
            "author",
            "author_id",
            FieldKind::Relation(RelationRef {
                target: ModelName::new("User"),
                name: None,
                through: None,
                fields: vec![FieldName::new("authorId")],
                references: vec![FieldName::new("id")],
                on_delete: Some(ReferentialAction::Cascade),
                on_update: None,
                resolved: Some(0),
                span: Span::new(120, 160),
            }),
        ),
    );

    let mut models = IndexMap::new();
    models.insert(
        ModelName::new("User"),
        Model {
            name: ModelName::new("User"),
            table: "users".to_owned(),
            fields: user_fields,
            primary_key: PrimaryKey {
                fields: vec![FieldName::new("id")],
                name: None,
                span: Span::new(30, 33),
            },
            indexes: vec![IndexDef {
                db_name: "users_email_idx".to_owned(),
                targets: vec![IndexTarget::Field(FieldName::new("email"), SortOrder::Asc)],
                where_clause: None,
                span: Span::new(80, 95),
            }],
            uniques: vec![],
            docs: Some("A registered account.".to_owned()),
            span: Span::new(28, 100),
        },
    );
    models.insert(
        ModelName::new("Post"),
        Model {
            name: ModelName::new("Post"),
            table: "posts".to_owned(),
            fields: post_fields,
            primary_key: PrimaryKey {
                fields: vec![FieldName::new("id")],
                name: None,
                span: Span::new(110, 113),
            },
            indexes: vec![],
            uniques: vec![],
            docs: None,
            span: Span::new(105, 170),
        },
    );

    Schema {
        version: IR_VERSION,
        datasource: Datasource {
            name: "db".to_owned(),
            provider: Provider::Postgres,
            url: DatasourceUrl::Env("DATABASE_URL".to_owned()),
            extensions: Vec::new(),
            span: Span::new(0, 4),
        },
        generator: Generator::default(),
        enums,
        models,
        relations: vec![ResolvedRelation {
            name: "PostToUser".to_owned(),
            kind: RelationKind::ManyToOne,
            owner: ModelName::new("Post"),
            owner_cols: vec!["author_id".to_owned()],
            owner_field: FieldName::new("author"),
            target: ModelName::new("User"),
            target_table: "users".to_owned(),
            target_cols: vec!["id".to_owned()],
            target_field: Some(FieldName::new("posts")),
            on_delete: ReferentialAction::Cascade,
            on_update: ReferentialAction::NoAction,
            optional: false,
            constraint_name: "posts_author_id_fkey".to_owned(),
            span: Span::new(120, 160),
            join_model: None,
            join_owner_field: None,
            join_target_field: None,
        }],
    }
}

#[test]
fn schema_round_trips_through_json() {
    let original = blog_schema();
    let json = serde_json::to_string_pretty(&original).expect("schema serialises");
    let restored: Schema = serde_json::from_str(&json).expect("schema deserialises");
    assert_eq!(original, restored);
}

#[test]
fn fingerprint_is_stable_and_content_sensitive() {
    let a = blog_schema();
    let b = blog_schema();
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_eq!(a.fingerprint().len(), 64);

    let mut c = blog_schema();
    c.models.get_mut("User").unwrap().table.push_str("_renamed");
    assert_ne!(a.fingerprint(), c.fingerprint());
}

#[test]
fn declaration_order_survives_serialisation() {
    // Stable ordering is a correctness requirement for the migration differ, not
    // a cosmetic one: a reordered IR would produce a spurious diff every run.
    let json = serde_json::to_string(&blog_schema()).unwrap();
    let restored: Schema = serde_json::from_str(&json).unwrap();

    let models: Vec<_> = restored.models.keys().map(ModelName::as_str).collect();
    assert_eq!(models, ["User", "Post"]);

    let fields: Vec<_> = restored
        .model("User")
        .unwrap()
        .fields
        .keys()
        .map(FieldName::as_str)
        .collect();
    assert_eq!(fields, ["id", "email", "name", "role", "posts"]);

    let variants: Vec<_> = restored
        .enum_def("Role")
        .unwrap()
        .variants
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(variants, ["USER", "ADMIN"]);
}

#[test]
fn relation_sides_resolve_to_one_canonical_entry() {
    let schema = blog_schema();

    let post_author = schema.model("Post").unwrap().field("author").unwrap();
    let user_posts = schema.model("User").unwrap().field("posts").unwrap();

    let from_owner = schema.relation(post_author.relation().unwrap()).unwrap();
    let from_target = schema.relation(user_posts.relation().unwrap()).unwrap();

    // Both sides must land on the same relation, or they could disagree about
    // foreign keys and referential actions.
    assert_eq!(from_owner, from_target);
    assert_eq!(from_owner.owner.as_str(), "Post");
    assert_eq!(from_owner.on_delete, ReferentialAction::Cascade);
}

#[test]
fn column_bearing_fields_are_distinguished_from_navigation_properties() {
    let schema = blog_schema();
    let user = schema.model("User").unwrap();

    let scalars: Vec<_> = user.scalar_fields().map(|f| f.name.as_str()).collect();
    assert_eq!(scalars, ["id", "email", "name", "role"]);

    // `posts` is a list-valued navigation property: no column on `users`.
    let relations: Vec<_> = user.relation_fields().map(|f| f.name.as_str()).collect();
    assert_eq!(relations, ["posts"]);
    assert!(!user.field("posts").unwrap().has_column());

    // `Post.author` is a navigation property; the FK lives on the `authorId`
    // scalar field.
    let post = schema.model("Post").unwrap();
    assert!(!post.field("author").unwrap().has_column());
    assert!(post.field("authorId").unwrap().has_column());
    assert_eq!(post.field("authorId").unwrap().column, "author_id");
}
