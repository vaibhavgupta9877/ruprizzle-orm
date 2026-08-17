//! The intermediate representation: the contract every other crate speaks.
//!
//! A [`Schema`] is the fully validated, **dialect-agnostic** description of the
//! user's `schema.ruprizzle`. Everything downstream — codegen, migrations, the
//! dialects — consumes only this type. Nothing downstream re-reads the source
//! text or re-applies naming rules.
//!
//! Two properties of this module are load-bearing and should not be relaxed:
//!
//! 1. **Order is stable.** Every collection is an [`IndexMap`] or [`Vec`], never a
//!    `HashMap`. Declaration order determines generated-code order and migration
//!    diff order; a hash map would make both churn between runs on the same input.
//! 2. **Physical names are resolved.** [`Model::table`] and [`Field::column`] are
//!    computed once during lowering (applying `@@map`/`@map` and the naming
//!    convention). No consumer needs to know the convention.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::names::{EnumName, FieldName, ModelName};
use crate::span::Span;

/// Version of the IR/snapshot format.
///
/// Bumped whenever a change to these types would make an existing
/// `migrations/.snapshot.json` misread. The migration engine refuses to load a
/// snapshot whose version it does not recognise.
pub const IR_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Root
// ---------------------------------------------------------------------------

/// A validated schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Format version, see [`IR_VERSION`].
    #[serde(default = "default_ir_version")]
    pub version: u32,
    /// The `datasource` block.
    pub datasource: Datasource,
    /// The `generator` block.
    pub generator: Generator,
    /// Enum declarations, in declaration order.
    pub enums: IndexMap<EnumName, EnumDef>,
    /// Model declarations, in declaration order.
    pub models: IndexMap<ModelName, Model>,
    /// Canonical relations.
    ///
    /// Both sides of a relation refer to the same entry by index, which is what
    /// makes it impossible for the two sides to disagree about foreign keys or
    /// referential actions. Populated during lowering (P1 pass 3).
    #[serde(default)]
    pub relations: Vec<ResolvedRelation>,
}

fn default_ir_version() -> u32 {
    IR_VERSION
}

fn empty_string() -> String {
    String::new()
}

impl Schema {
    /// Looks up a model by name.
    #[must_use]
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.models.get(name)
    }

    /// Looks up an enum by name.
    #[must_use]
    pub fn enum_def(&self, name: &str) -> Option<&EnumDef> {
        self.enums.get(name)
    }

    /// Resolves a [`RelationRef`] to its canonical relation.
    ///
    /// Returns `None` for a reference that has not been resolved yet, which can
    /// only happen mid-lowering.
    #[must_use]
    pub fn relation(&self, r: &RelationRef) -> Option<&ResolvedRelation> {
        r.resolved.and_then(|i| self.relations.get(i))
    }

    /// A stable content hash of the schema.
    ///
    /// Written into generated files and migration metadata so the CLI can detect
    /// that generated code is stale relative to the schema it came from.
    ///
    /// # Panics
    ///
    /// Panics only if the IR fails to serialise, which cannot happen: every type
    /// in this module derives `Serialize` over plain data with no custom
    /// implementations that could error.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let canonical = serde_json::to_vec(self).expect("IR is always serialisable");
        let digest = Sha256::digest(&canonical);
        digest.iter().fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{b:02x}");
            acc
        })
    }
}

// ---------------------------------------------------------------------------
// Configuration blocks
// ---------------------------------------------------------------------------

/// The `datasource` block: which database, and how to reach it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Datasource {
    /// Block name as written, e.g. `db`.
    pub name: String,
    /// Which SQL dialect this schema targets.
    pub provider: Provider,
    /// Where the connection string comes from.
    pub url: DatasourceUrl,
    /// Source location of the `datasource` block.
    pub span: Span,
}

/// A supported SQL dialect.
///
/// Adding a variant here is the first step of adding a dialect; the compiler then
/// points at every place that must handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// `PostgreSQL` 14 or newer.
    Postgres,
    /// `SQLite` 3.35 or newer, the first version with `DROP COLUMN` and `RETURNING`.
    Sqlite,
    /// `MySQL` 8.0+ and `MariaDB` 10.5+.
    Mysql,
}

impl Provider {
    /// Parses the string used in `provider = "..."`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "postgres" | "postgresql" => Some(Provider::Postgres),
            "sqlite" => Some(Provider::Sqlite),
            "mysql" | "mariadb" => Some(Provider::Mysql),
            _ => None,
        }
    }

    /// The canonical spelling, as it appears in generated files and errors.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Provider::Postgres => "postgres",
            Provider::Sqlite => "sqlite",
            Provider::Mysql => "mysql",
        }
    }

    /// Every provider this build supports, for error suggestions.
    pub const ALL: &'static [Provider] = &[Provider::Postgres, Provider::Sqlite, Provider::Mysql];
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the connection URL is supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatasourceUrl {
    /// `url = env("DATABASE_URL")` — resolved at run time, never stored.
    Env(String),
    /// `url = "postgres://..."` — inline. Discouraged; the CLI warns.
    Literal(String),
}

/// The `generator` block: where and how to emit Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generator {
    /// Block name as written, e.g. `client`.
    pub name: String,
    /// Output directory, relative to the schema file.
    pub output: String,
    /// Module name the generated code expects to be mounted as.
    pub module_name: String,
    /// Maximum `include` nesting depth accepted by generated builders.
    pub max_include_depth: usize,
    /// Source location of the `generator` block.
    pub span: Span,
}

impl Default for Generator {
    fn default() -> Self {
        Generator {
            name: "client".to_owned(),
            output: "src/db".to_owned(),
            module_name: "db".to_owned(),
            max_include_depth: 3,
            span: Span::EMPTY,
        }
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// An `enum` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDef {
    /// Name as written in the schema.
    pub name: EnumName,
    /// Physical type name (Postgres) or CHECK-constraint basis (`SQLite`).
    pub db_name: String,
    /// Variants in declaration order. Order is significant: Postgres orders enum
    /// values by declaration, and `ORDER BY` on an enum column follows it.
    pub variants: IndexMap<String, EnumVariant>,
    /// Doc comments, emitted as rustdoc on the generated Rust enum.
    pub docs: Option<String>,
    /// Source location of the declaration.
    pub span: Span,
}

/// One variant of an [`EnumDef`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    /// Name as written, e.g. `ADMIN`.
    pub name: String,
    /// Value actually stored in the database.
    pub db_name: String,
    /// Doc comments, emitted as rustdoc on the generated variant.
    pub docs: Option<String>,
    /// Source location of the variant.
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// A `model` declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    /// Name as written in the schema.
    pub name: ModelName,
    /// Physical table name.
    pub table: String,
    /// Fields in declaration order.
    pub fields: IndexMap<FieldName, Field>,
    /// The primary key. Always present: validation rejects models without one.
    pub primary_key: PrimaryKey,
    /// Index declarations, in declaration order.
    pub indexes: Vec<IndexDef>,
    /// Unique-constraint declarations, in declaration order.
    pub uniques: Vec<UniqueDef>,
    /// `///` doc comments, emitted as rustdoc on the generated struct.
    pub docs: Option<String>,
    /// Source location of the declaration.
    pub span: Span,
}

impl Model {
    /// Looks up a field by name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.get(name)
    }

    /// Fields that map to a physical column, in declaration order.
    ///
    /// Excludes list-valued navigation properties, which have no column.
    pub fn scalar_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields.values().filter(|f| f.has_column())
    }

    /// Navigation properties, in declaration order.
    pub fn relation_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields.values().filter(|f| f.relation().is_some())
    }
}

/// A field within a [`Model`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// Name as written in the schema.
    pub name: FieldName,
    /// Physical column name. Meaningless for list-valued relations, which have
    /// no column of their own; see [`Field::has_column`].
    pub column: String,
    /// What the field holds.
    pub kind: FieldKind,
    /// `?` in the DSL. Maps to `Option<T>` in Rust and `NULL` in SQL.
    pub optional: bool,
    /// The declared default, if given.
    pub default: Option<DefaultValue>,
    /// Remaining attributes: identity, uniqueness, rename hints, and so on.
    pub attrs: FieldAttrs,
    /// Doc comments, emitted as rustdoc on the generated struct field.
    pub docs: Option<String>,
    /// Source location of the field.
    pub span: Span,
}

impl Field {
    /// Whether this field corresponds to a physical column.
    ///
    /// `false` for navigation properties (`Post[]`, the owner side of a relation,
    /// or the non-owning side of a 1:1). The foreign key columns of a relation
    /// are the scalar fields named in the relation's `fields:`. Scalar and enum
    /// lists (`String[]`, `Role[]`) do have a column.
    #[must_use]
    pub fn has_column(&self) -> bool {
        match &self.kind {
            FieldKind::Scalar(_) | FieldKind::Enum(_) => true,
            FieldKind::List(inner) => {
                matches!(inner.as_ref(), FieldKind::Scalar(_) | FieldKind::Enum(_))
            }
            FieldKind::Relation(_) => false,
        }
    }

    /// The relation this field navigates, if any — including through a list.
    #[must_use]
    pub fn relation(&self) -> Option<&RelationRef> {
        match &self.kind {
            FieldKind::Relation(r) => Some(r),
            FieldKind::List(inner) => match inner.as_ref() {
                FieldKind::Relation(r) => Some(r),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether the field is a list (`T[]`).
    #[must_use]
    pub fn is_list(&self) -> bool {
        matches!(self.kind, FieldKind::List(_))
    }
}

/// What a field holds.
///
/// A field is *either* scalar data *or* a relation. Never both — the foreign key
/// column and the navigation property are two separate fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldKind {
    /// A built-in scalar column.
    Scalar(ScalarType),
    /// A column typed by a schema enum.
    Enum(EnumName),
    /// A navigation property. The FK column lives on whichever side declares
    /// `fields:`; see `ImplPlan06RelationsInclude.md`.
    Relation(RelationRef),
    /// `Post[]` — the "many" side, has no column of its own.
    List(Box<FieldKind>),
}

/// A built-in scalar type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScalarType {
    /// UTF-8 text. Rust `String`.
    String,
    /// 32-bit signed integer. Rust `i32`.
    Int,
    /// 64-bit signed integer. Rust `i64`.
    BigInt,
    /// 64-bit floating point. Rust `f64`. Not for money.
    Float,
    /// Exact numeric. Rust `rust_decimal::Decimal`.
    ///
    /// `SQLite` has no exact numeric type and stores this as text; codegen warns
    /// when that applies.
    Decimal,
    /// True or false. Rust `bool`.
    Boolean,
    /// An instant, always stored and returned as UTC.
    DateTime,
    /// A calendar date, with no time or zone.
    Date,
    /// A wall-clock time, with no date or zone.
    Time,
    /// A UUID. Rust `uuid::Uuid`.
    Uuid,
    /// Arbitrary JSON. Rust `serde_json::Value`.
    Json,
    /// Opaque binary. Rust `Vec<u8>`.
    Bytes,
}

impl ScalarType {
    /// Parses the spelling used in the DSL.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        use ScalarType::{
            BigInt, Boolean, Bytes, Date, DateTime, Decimal, Float, Int, Json, String, Time, Uuid,
        };
        Some(match s {
            "String" => String,
            "Int" => Int,
            "BigInt" => BigInt,
            "Float" => Float,
            "Decimal" => Decimal,
            "Boolean" => Boolean,
            "DateTime" => DateTime,
            "Date" => Date,
            "Time" => Time,
            "Uuid" => Uuid,
            "Json" => Json,
            "Bytes" => Bytes,
            _ => return None,
        })
    }

    /// The spelling used in the DSL.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        use ScalarType::{
            BigInt, Boolean, Bytes, Date, DateTime, Decimal, Float, Int, Json, String, Time, Uuid,
        };
        match self {
            String => "String",
            Int => "Int",
            BigInt => "BigInt",
            Float => "Float",
            Decimal => "Decimal",
            Boolean => "Boolean",
            DateTime => "DateTime",
            Date => "Date",
            Time => "Time",
            Uuid => "Uuid",
            Json => "Json",
            Bytes => "Bytes",
        }
    }

    /// Every scalar type, for error suggestions and exhaustive tests.
    pub const ALL: &'static [ScalarType] = &[
        ScalarType::String,
        ScalarType::Int,
        ScalarType::BigInt,
        ScalarType::Float,
        ScalarType::Decimal,
        ScalarType::Boolean,
        ScalarType::DateTime,
        ScalarType::Date,
        ScalarType::Time,
        ScalarType::Uuid,
        ScalarType::Json,
        ScalarType::Bytes,
    ];
}

impl std::fmt::Display for ScalarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Per-field attributes that are not defaults or types.
///
/// The boolean flags mirror the DSL's independent marker attributes one-for-one.
/// Collapsing them into an enum or bitflags would misrepresent the domain —
/// `@id` and `@unique` genuinely can coexist — and would make the mapping from
/// source attribute to IR field indirect for no gain.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FieldAttrs {
    /// `@id`
    pub is_id: bool,
    /// `@unique`
    pub is_unique: bool,
    /// `@updatedAt` — set by the update builder in Rust, not by a DB trigger, so
    /// behaviour is identical across dialects.
    pub is_updated_at: bool,
    /// `@ignore` — present in the database, absent from generated code.
    pub ignore: bool,
    /// `@db.VarChar(200)` and friends.
    pub native_type: Option<NativeType>,
    /// `@renamedFrom("old")` — authoritative rename hint for the migration differ.
    pub renamed_from: Option<String>,
}

/// A dialect-specific column type override, e.g. `@db.VarChar(200)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeType {
    /// Type name as written after `@db.`, e.g. `VarChar`.
    pub name: String,
    /// Positional arguments, e.g. `["200"]`.
    pub args: Vec<String>,
    /// Source location of the attribute.
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Keys, indexes, defaults
// ---------------------------------------------------------------------------

/// A model's primary key. Always present; validation rejects models without one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimaryKey {
    /// One field for `@id`, several for `@@id([a, b])`.
    pub fields: Vec<FieldName>,
    /// Explicit constraint name, if given.
    /// Explicit constraint name, if given.
    pub name: Option<String>,
    /// Source location of the declaration that introduced the key.
    pub span: Span,
}

impl PrimaryKey {
    /// Whether the key spans more than one column.
    #[must_use]
    pub fn is_composite(&self) -> bool {
        self.fields.len() > 1
    }
}

/// An `@@index([...])` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDef {
    /// Physical index name, derived or explicit.
    pub db_name: String,
    /// Indexed columns, in index order — which determines what the index can
    /// serve, so it is never sorted.
    pub fields: Vec<IndexField>,
    /// Source location of the declaration.
    pub span: Span,
}

/// One column within an index, with its sort direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexField {
    /// The indexed field.
    pub field: FieldName,
    /// Sort direction for this column within the index.
    pub order: SortOrder,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortOrder {
    /// Ascending.
    #[default]
    Asc,
    /// Descending.
    Desc,
}

/// An `@@unique([...])` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UniqueDef {
    /// Physical constraint name, derived or explicit.
    pub db_name: String,
    /// Columns covered by the constraint, in declaration order.
    pub fields: Vec<FieldName>,
    /// Source location of the declaration.
    pub span: Span,
}

/// A `@default(...)` value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DefaultValue {
    /// A constant written in the schema.
    Literal(Literal),
    /// A function default, resolved per-dialect.
    Function(DefaultFn),
    /// `dbgenerated("...")` — passed through verbatim; the dialect's problem.
    DbGenerated(String),
}

/// A literal in the DSL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    /// A quoted string.
    String(String),
    /// An integer.
    Int(i64),
    /// A floating-point number.
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// An enum variant name, e.g. `@default(USER)`.
    EnumVariant(String),
}

/// A built-in default function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefaultFn {
    /// Random v4 UUID.
    Uuid4,
    /// Time-ordered v7 UUID. Preferred for primary keys: monotonic values avoid
    /// the B-tree page splits that random keys cause on insert-heavy tables.
    Uuid7,
    /// Collision-resistant sortable identifier, generated client-side.
    Cuid2,
    /// Compact URL-safe identifier, generated client-side.
    Nanoid,
    /// Current timestamp.
    Now,
    /// Database-assigned increasing integer.
    AutoIncrement,
}

impl DefaultFn {
    /// Parses the spelling used in `@default(...)`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "uuid4" => DefaultFn::Uuid4,
            "uuid7" => DefaultFn::Uuid7,
            "cuid2" => DefaultFn::Cuid2,
            "nanoid" => DefaultFn::Nanoid,
            "now" => DefaultFn::Now,
            "autoincrement" => DefaultFn::AutoIncrement,
            _ => return None,
        })
    }

    /// The spelling used in the DSL.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            DefaultFn::Uuid4 => "uuid4",
            DefaultFn::Uuid7 => "uuid7",
            DefaultFn::Cuid2 => "cuid2",
            DefaultFn::Nanoid => "nanoid",
            DefaultFn::Now => "now",
            DefaultFn::AutoIncrement => "autoincrement",
        }
    }

    /// Whether the value is produced by the client rather than the database.
    ///
    /// Client-side defaults must be supplied by the insert builder, because no
    /// `DEFAULT` clause exists in the DDL for them.
    #[must_use]
    pub const fn is_client_side(&self) -> bool {
        matches!(
            self,
            DefaultFn::Uuid7 | DefaultFn::Cuid2 | DefaultFn::Nanoid
        )
    }

    /// Every default function, for error suggestions.
    pub const ALL: &'static [DefaultFn] = &[
        DefaultFn::Uuid4,
        DefaultFn::Uuid7,
        DefaultFn::Cuid2,
        DefaultFn::Nanoid,
        DefaultFn::Now,
        DefaultFn::AutoIncrement,
    ];
}

// ---------------------------------------------------------------------------
// Relations
// ---------------------------------------------------------------------------

/// A relation as written on one field.
///
/// This is the *syntactic* half. After lowering, [`RelationRef::resolved`] indexes
/// into [`Schema::relations`], where both sides meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationRef {
    /// The model on the other end.
    pub target: ModelName,
    /// Explicit `@relation("name")`, required when two relations connect the same
    /// pair of models.
    pub name: Option<String>,
    /// Join model for an explicit many-to-many (`@relation(through: PostTag)`).
    /// Only valid on list-valued relation fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through: Option<ModelName>,
    /// FK fields on *this* model. Non-empty only on the owning side.
    pub fields: Vec<FieldName>,
    /// Referenced fields on the target model.
    pub references: Vec<FieldName>,
    /// Delete behaviour, if given. Defaults are applied during lowering.
    pub on_delete: Option<ReferentialAction>,
    /// Update behaviour, if given. Defaults are applied during lowering.
    pub on_update: Option<ReferentialAction>,
    /// Index into [`Schema::relations`], filled in during lowering.
    #[serde(default)]
    pub resolved: Option<usize>,
    /// Source location of the field carrying the relation.
    pub span: Span,
}

/// A relation after both sides have been paired up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRelation {
    /// Explicit or derived relation name; unique within the schema.
    pub name: String,
    /// Cardinality, from the owner's point of view.
    pub kind: RelationKind,
    /// The model holding the foreign key.
    pub owner: ModelName,
    /// FK columns on the owner, in order.
    pub owner_cols: Vec<String>,
    /// Field on the owner that navigates to the target.
    pub owner_field: FieldName,
    /// The referenced model.
    pub target: ModelName,
    /// Physical table name of the referenced model.
    #[serde(default = "empty_string")]
    pub target_table: String,
    /// Referenced columns on the target, in order, positionally matching
    /// `owner_cols`.
    pub target_cols: Vec<String>,
    /// Back-reference field on the target, if one was declared.
    pub target_field: Option<FieldName>,
    /// What happens to owner rows when the referenced row is deleted.
    pub on_delete: ReferentialAction,
    /// What happens to the foreign key when the referenced key changes.
    pub on_update: ReferentialAction,
    /// Whether the FK is nullable.
    pub optional: bool,
    /// Physical constraint name.
    pub constraint_name: String,
    /// Source location of the owning side's relation attribute.
    pub span: Span,
    /// Join model for many-to-many relations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_model: Option<ModelName>,
    /// Field in the join model that references the owner endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_owner_field: Option<FieldName>,
    /// Field in the join model that references the target endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_target_field: Option<FieldName>,
}

/// The cardinality of a relation, from the owner's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationKind {
    /// Owner holds a unique FK: one owner row per target row.
    OneToOne,
    /// Owner holds a non-unique FK: many owner rows per target row.
    ManyToOne,
    /// Both sides are lists, joined by an explicit model in between.
    ManyToMany,
}

/// `onDelete` / `onUpdate` behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ReferentialAction {
    /// Propagate the delete or update to the referencing rows.
    Cascade,
    /// Reject the operation immediately if referencing rows exist.
    Restrict,
    /// Null out the foreign key. Requires a nullable column.
    SetNull,
    /// Reset the foreign key to its column default.
    SetDefault,
    /// Reject the operation, deferrably. The SQL default.
    #[default]
    NoAction,
}

impl ReferentialAction {
    /// Parses the spelling used in `@relation(onDelete: ...)`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "Cascade" => ReferentialAction::Cascade,
            "Restrict" => ReferentialAction::Restrict,
            "SetNull" => ReferentialAction::SetNull,
            "SetDefault" => ReferentialAction::SetDefault,
            "NoAction" => ReferentialAction::NoAction,
            _ => return None,
        })
    }

    /// The SQL clause fragment.
    #[must_use]
    pub const fn as_sql(&self) -> &'static str {
        match self {
            ReferentialAction::Cascade => "CASCADE",
            ReferentialAction::Restrict => "RESTRICT",
            ReferentialAction::SetNull => "SET NULL",
            ReferentialAction::SetDefault => "SET DEFAULT",
            ReferentialAction::NoAction => "NO ACTION",
        }
    }

    /// Every action, for error suggestions.
    pub const ALL: &'static [ReferentialAction] = &[
        ReferentialAction::Cascade,
        ReferentialAction::Restrict,
        ReferentialAction::SetNull,
        ReferentialAction::SetDefault,
        ReferentialAction::NoAction,
    ];
}
