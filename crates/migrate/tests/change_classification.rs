//! `Change::is_destructive` and the enum / rename parts of `diff` (K9).
//!
//! `cargo mutants` showed that `is_destructive` could be replaced by `true`,
//! and all of `diff_enums` by `()`, without a test failing. The destructive
//! flag decides whether `migrate` asks for `--accept-data-loss`, so a safe
//! change reported as destructive blocks deploys, and the reverse loses data.

use ruprizzle_core::ir::Schema;
use ruprizzle_core::names::{EnumName, FieldName, ModelName};
use ruprizzle_migrate::{Change, ColumnAspect, diff};
use ruprizzle_parser::parse;

const HEADER: &str = r#"
datasource db {
    provider = "postgresql"
    url      = "postgres://localhost/db"
}

generator client {
    provider = "rust"
}
"#;

fn schema(body: &str) -> Schema {
    parse("test", &format!("{HEADER}{body}")).expect("schema parses")
}

/// One schema holding an example of every IR piece a `Change` can carry.
fn rich() -> Schema {
    schema(
        r#"
enum Role {
    ADMIN
    MEMBER
}

model User {
    id    Int    @id
    email String @unique
    role  Role
    posts Post[]

    @@index([role])
    @@unique([email, role])
}

model Post {
    id       Int  @id
    authorId Int
    author   User @relation(fields: [authorId], references: [id])
}
"#,
    )
}

/// Every `Change` variant, each paired with whether it can delete data.
fn every_variant() -> Vec<(Change, bool)> {
    let s = rich();
    let user = s.models.get("User").expect("User").clone();
    let email = user.fields.get("email").expect("email").clone();
    let role = s.enums.get("Role").expect("Role").clone();
    let index = user.indexes.first().expect("an index").clone();
    let unique = user.uniques.first().expect("a unique").clone();
    let fk = s.relations.first().expect("a relation").clone();
    let m = || ModelName::from("User");

    vec![
        (Change::CreateEnum(role), false),
        (
            Change::DropEnum(EnumName::from("Role"), "role".into()),
            true,
        ),
        (
            Change::AddEnumVariant {
                enum_: EnumName::from("Role"),
                variant: "GUEST".into(),
            },
            false,
        ),
        (
            Change::DropEnumVariant {
                enum_: EnumName::from("Role"),
                variant: "GUEST".into(),
            },
            true,
        ),
        (Change::CreateModel(user.clone()), false),
        (Change::DropModel(m(), "users".into()), true),
        (
            Change::RenameModel {
                from: m(),
                to: ModelName::from("Account"),
                new_table: "accounts".into(),
            },
            false,
        ),
        (
            Change::AddColumn {
                model: m(),
                field: email.clone(),
            },
            false,
        ),
        (
            Change::DropColumn {
                model: m(),
                column: "email".into(),
            },
            true,
        ),
        (
            Change::AlterColumn {
                model: m(),
                from: email.clone(),
                to: email,
                aspects: Vec::new(),
            },
            false,
        ),
        (
            Change::RenameColumn {
                model: m(),
                from: FieldName::from("email"),
                to: FieldName::from("mail"),
                from_column: "email".into(),
                to_column: "mail".into(),
            },
            false,
        ),
        (Change::CreateIndex(m(), index), false),
        (Change::DropIndex(m(), "ix".into()), true),
        (Change::AddUnique(m(), unique), false),
        (Change::DropUnique(m(), "uq".into()), true),
        (
            Change::AddForeignKey(ModelName::from("Post"), fk.clone()),
            false,
        ),
        (Change::DropForeignKey(ModelName::from("Post"), fk), true),
    ]
}

#[test]
fn every_change_variant_has_the_expected_destructive_flag() {
    let cases = every_variant();
    // 17 variants: keep this in step with `Change` so a new variant is classified here.
    assert_eq!(cases.len(), 17);
    for (change, destructive) in cases {
        assert_eq!(
            change.is_destructive(),
            destructive,
            "{}",
            change.description()
        );
    }
}

#[test]
fn a_purely_additive_migration_is_not_destructive() {
    let prev = schema(
        r#"
model User {
    id Int @id
}
"#,
    );
    let next = rich();
    let changes = diff(&prev, &next);
    assert!(!changes.is_empty());
    assert!(
        changes.iter().all(|c| !c.is_destructive()),
        "{:?}",
        changes.iter().map(Change::description).collect::<Vec<_>>()
    );
}

fn enum_changes(changes: &[Change]) -> Vec<String> {
    changes
        .iter()
        .filter(|c| {
            matches!(
                c,
                Change::CreateEnum(_)
                    | Change::DropEnum(..)
                    | Change::AddEnumVariant { .. }
                    | Change::DropEnumVariant { .. }
            )
        })
        .map(Change::description)
        .collect()
}

const ROLE_MODEL: &str = r#"
model User {
    id   Int    @id
    role String
}
"#;

#[test]
fn adding_an_enum_creates_it() {
    let prev = schema(ROLE_MODEL);
    let next = schema(&format!(
        "enum Color {{\n    RED\n    BLUE\n}}\n{ROLE_MODEL}"
    ));
    let changes = diff(&prev, &next);
    assert_eq!(enum_changes(&changes), vec!["CREATE ENUM Color"]);
    assert!(changes.iter().all(|c| !c.is_destructive()));
}

#[test]
fn removing_an_enum_drops_it_and_is_destructive() {
    let prev = schema(&format!(
        "enum Color {{\n    RED\n    BLUE\n}}\n{ROLE_MODEL}"
    ));
    let next = schema(ROLE_MODEL);
    let changes = diff(&prev, &next);
    assert_eq!(enum_changes(&changes), vec!["DROP ENUM Color"]);
    assert!(changes.iter().any(Change::is_destructive));
}

#[test]
fn enum_variants_are_added_and_dropped() {
    let prev = schema(&format!(
        "enum Color {{\n    RED\n    BLUE\n}}\n{ROLE_MODEL}"
    ));
    let next = schema(&format!(
        "enum Color {{\n    RED\n    GREEN\n}}\n{ROLE_MODEL}"
    ));
    let changes = diff(&prev, &next);
    assert_eq!(
        enum_changes(&changes),
        vec![
            "ADD ENUM VARIANT Color.GREEN",
            "DROP ENUM VARIANT Color.BLUE"
        ]
    );
}

#[test]
fn an_unchanged_enum_produces_no_change() {
    let s = schema(&format!(
        "enum Color {{\n    RED\n    BLUE\n}}\n{ROLE_MODEL}"
    ));
    assert!(diff(&s, &s).is_empty());
}

/// `@renamedFrom` naming a relation field cannot become a column rename: the
/// old field has no column. The new field is added instead.
#[test]
fn a_rename_hint_from_a_relation_field_is_not_a_column_rename() {
    let prev = schema(
        r#"
model User {
    id    Int    @id
    posts Post[]
}

model Post {
    id       Int  @id
    authorId Int
    author   User @relation(fields: [authorId], references: [id])
}
"#,
    );
    let next = schema(
        r#"
model User {
    id    Int    @id
    posts Post[]
}

model Post {
    id       Int    @id
    authorId Int
    author   User   @relation(fields: [authorId], references: [id])
    writer   String @renamedFrom("author")
}
"#,
    );
    let changes = diff(&prev, &next);
    assert!(
        !changes
            .iter()
            .any(|c| matches!(c, Change::RenameColumn { .. })),
        "{:?}",
        changes.iter().map(Change::description).collect::<Vec<_>>()
    );
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::AddColumn { field, .. } if field.name.as_str() == "writer"
    )));
}

/// A renamed field is compared against the field it was renamed from, never
/// against an unrelated old field that happens to share its new name.
///
/// This pins only the "no alter" half. The old `b` is not dropped either, so
/// the planned `RENAME COLUMN a TO b` collides with it; that gap is tracked
/// separately (PathToV1_5 K11).
#[test]
fn a_renamed_field_is_not_altered_against_a_namesake() {
    let prev = schema(
        r#"
model User {
    id Int    @id
    a  Int
    b  String
}
"#,
    );
    let next = schema(
        r#"
model User {
    id Int @id
    b  Int @renamedFrom("a")
}
"#,
    );
    let changes = diff(&prev, &next);
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::RenameColumn { from, to, .. } if from.as_str() == "a" && to.as_str() == "b"
    )));
    assert!(
        !changes
            .iter()
            .any(|c| matches!(c, Change::AlterColumn { .. })),
        "{:?}",
        changes.iter().map(Change::description).collect::<Vec<_>>()
    );
}

#[test]
fn an_unchanged_schema_produces_no_change() {
    // Covers indexes, uniques and relations: a flipped comparison in any of
    // them reports a change between two identical schemas.
    assert_eq!(
        diff(&rich(), &rich())
            .iter()
            .map(Change::description)
            .collect::<Vec<_>>(),
        Vec::<String>::new()
    );
}

fn aspects_of(prev_field: &str, next_field: &str) -> Vec<Vec<ColumnAspect>> {
    let model = |field: &str| {
        format!(
            "enum Role {{\n    A\n}}\nenum Tier {{\n    B\n}}\nmodel User {{\n    id Int @id\n    {field}\n}}\n"
        )
    };
    diff(&schema(&model(prev_field)), &schema(&model(next_field)))
        .into_iter()
        .filter_map(|c| match c {
            Change::AlterColumn { aspects, .. } => Some(aspects),
            _ => None,
        })
        .collect()
}

#[test]
fn column_changes_report_the_aspect_that_changed() {
    use ColumnAspect::{Default, Nullability, Type};
    assert_eq!(
        aspects_of("x Int", "x Int"),
        Vec::<Vec<ColumnAspect>>::new()
    );
    assert_eq!(aspects_of("x Int", "x String"), vec![vec![Type]]);
    assert_eq!(aspects_of("x Role", "x Tier"), vec![vec![Type]]);
    assert_eq!(
        aspects_of("x String", "x String @db.VarChar(20)"),
        vec![vec![Type]]
    );
    assert_eq!(aspects_of("x Int", "x Int?"), vec![vec![Nullability]]);
    assert_eq!(
        aspects_of("x Int", "x Int @default(0)"),
        vec![vec![Default]]
    );
    assert_eq!(
        aspects_of("x Int", "x String? @default(\"a\")"),
        vec![vec![Type, Nullability, Default]]
    );
    // Altering a column is never destructive on its own.
    assert!(
        diff(
            &schema("model User {\n    id Int @id\n    x Int\n}\n"),
            &schema("model User {\n    id Int @id\n    x String\n}\n"),
        )
        .iter()
        .all(|c| !c.is_destructive())
    );
}

#[test]
fn indexes_and_uniques_are_created_and_dropped() {
    let plain = schema("model User {\n    id Int @id\n    a Int\n    b Int\n}\n");
    let keyed = schema(
        "model User {\n    id Int @id\n    a Int\n    b Int\n\n    @@index([a])\n    @@unique([a, b])\n}\n",
    );
    let names = |changes: Vec<Change>| -> Vec<(bool, &'static str)> {
        changes
            .iter()
            .map(|c| match c {
                Change::CreateIndex(..) => (c.is_destructive(), "create index"),
                Change::DropIndex(..) => (c.is_destructive(), "drop index"),
                Change::AddUnique(..) => (c.is_destructive(), "add unique"),
                Change::DropUnique(..) => (c.is_destructive(), "drop unique"),
                _ => (c.is_destructive(), "other"),
            })
            .collect()
    };
    assert_eq!(
        names(diff(&plain, &keyed)),
        vec![(false, "create index"), (false, "add unique")]
    );
    assert_eq!(
        names(diff(&keyed, &plain)),
        vec![(true, "drop index"), (true, "drop unique")]
    );
}

const USER: &str = "model User {\n    id Int @id\n    posts Post[]\n}\n";

fn post(relation_args: &str) -> Schema {
    schema(&format!(
        "{USER}model Post {{\n    id Int @id\n    authorId Int\n    author User @relation(fields: [authorId], references: [id]{relation_args})\n}}\n"
    ))
}

fn fk_changes(prev: &Schema, next: &Schema) -> Vec<&'static str> {
    diff(prev, next)
        .iter()
        .filter_map(|c| match c {
            Change::AddForeignKey(..) => Some("add fk"),
            Change::DropForeignKey(..) => Some("drop fk"),
            _ => None,
        })
        .collect()
}

#[test]
fn changing_a_foreign_key_drops_and_recreates_it() {
    let base = post("");
    assert_eq!(fk_changes(&base, &base), Vec::<&str>::new());
    for changed in [", onDelete: Cascade", ", onUpdate: Restrict"] {
        assert_eq!(
            fk_changes(&base, &post(changed)),
            vec!["drop fk", "add fk"],
            "{changed}"
        );
    }
}

#[test]
fn foreign_keys_follow_their_relation() {
    let without = schema(&format!(
        "{}model Post {{\n    id Int @id\n    authorId Int\n}}\n",
        "model User {\n    id Int @id\n}\n"
    ));
    let with = post("");
    assert_eq!(fk_changes(&without, &with), vec!["add fk"]);
    assert_eq!(fk_changes(&with, &without), vec!["drop fk"]);

    // Dropping the owning table removes its foreign keys with it.
    let only_user = schema("model User {\n    id Int @id\n}\n");
    assert_eq!(fk_changes(&with, &only_user), Vec::<&str>::new());
}
