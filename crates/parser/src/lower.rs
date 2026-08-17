//! AST → IR lowering (P1-02), in five passes.
//!
//! ```text
//! ast::Schema ──▶ [1: collect names]   model / enum namespace
//!             ──▶ [2: lower types]     scalars, enums, model references
//!             ──▶ [3: resolve relations] pairs both sides, assigns the owner
//!             ──▶ [4: apply naming]    @map / @@map and the convention
//!             ──▶ [5: validate]        crate::validate
//! ```
//!
//! Pass 1 exists because a type may be referenced before it is declared, so a
//! single-pass walk cannot tell `Role` (enum) from `User` (model) from `String`
//! (scalar). Pass 3 is separate for the same reason at one level up: a relation
//! needs both models, and only one of them exists while the first is being
//! lowered.
//!
//! Lowering never stops at the first problem. Where a value cannot be resolved it
//! records a diagnostic, substitutes something harmless, and carries on, so one
//! run reports everything wrong with the file.
//!
//! # Rule coverage
//!
//! This module enforces V02–V10 and V12–V15; the rules that need only the
//! finished IR — V01, V11, V16, V17 — live in [`crate::validate`].

use indexmap::IndexMap;
use indexmap::map::Entry;
use ruprizzle_core::diagnostic::{Diagnostics, SchemaError};
use ruprizzle_core::ir::{
    Datasource, DatasourceUrl, DefaultFn, DefaultValue, EnumDef, EnumVariant, Field, FieldAttrs,
    FieldKind, Generator, IndexDef, IndexField, Literal, Model, NativeType, PrimaryKey, Provider,
    ReferentialAction, RelationKind, RelationRef, ResolvedRelation, ScalarType, Schema, SortOrder,
    UniqueDef,
};
use ruprizzle_core::names::{EnumName, FieldName, ModelName};
use ruprizzle_core::span::Span;
use ruprizzle_core::suggest;

use crate::ast::{Arity, Ast, Attr, EnumDecl, FieldDecl, ModelDecl, Value};
use crate::naming;

/// What a name in the schema's type namespace refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    Model,
    Enum,
}

/// The name environment built by pass 1.
struct NameEnv {
    kinds: IndexMap<String, (DeclKind, Span)>,
}

impl NameEnv {
    fn kind_of(&self, name: &str) -> Option<DeclKind> {
        self.kinds.get(name).map(|(k, _)| *k)
    }

    /// Everything a field type could legally name, for "did you mean…?".
    fn type_candidates(&self) -> Vec<String> {
        let mut out: Vec<String> = ScalarType::ALL
            .iter()
            .map(|s| (*s).as_str().to_owned())
            .collect();
        out.extend(self.kinds.keys().cloned());
        out
    }
}

/// Lowers a parsed schema to the IR, collecting every problem found on the way.
pub(crate) fn lower(ast: &Ast, diags: &mut Diagnostics) -> Schema {
    let env = collect_names(ast, diags);
    let datasource = lower_datasource(ast, diags);
    let generator = lower_generator(ast);
    let enums = lower_enums(ast, diags);
    let mut models = lower_models(ast, &env, &enums, diags);
    let relations = resolve_relations(&mut models, diags);

    let schema = Schema {
        version: ruprizzle_core::ir::IR_VERSION,
        datasource,
        generator,
        enums,
        models,
        relations,
    };
    crate::validate::validate(&schema, diags);
    schema
}

// ---------------------------------------------------------------------------
// Pass 1 — names
// ---------------------------------------------------------------------------

fn collect_names(ast: &Ast, diags: &mut Diagnostics) -> NameEnv {
    let mut kinds: IndexMap<String, (DeclKind, Span)> = IndexMap::new();

    let declared = ast
        .enums()
        .map(|e| (e.name.clone(), DeclKind::Enum, e.name_span))
        .chain(
            ast.models()
                .map(|m| (m.name.clone(), DeclKind::Model, m.name_span)),
        );

    for (name, kind, span) in declared {
        match kinds.entry(name.clone()) {
            Entry::Vacant(slot) => {
                slot.insert((kind, span));
            }
            // V03 — models and enums share one namespace.
            Entry::Occupied(existing) => diags.push(SchemaError::DuplicateDecl {
                name,
                span: span.into(),
                first: existing.get().1.into(),
            }),
        }
    }

    NameEnv { kinds }
}

// ---------------------------------------------------------------------------
// Configuration blocks
// ---------------------------------------------------------------------------

fn lower_datasource(ast: &Ast, diags: &mut Diagnostics) -> Datasource {
    let Some(block) = ast.datasources().next() else {
        diags.push(missing_block(
            "datasource",
            "add a `datasource db { provider = \"postgres\"  url = env(\"DATABASE_URL\") }` block",
        ));
        return fallback_datasource();
    };

    let provider = match block.get("provider").map(|e| &e.value) {
        Some(Value::Str(s, span)) => {
            if let Some(p) = Provider::parse(s) {
                p
            } else {
                // V15
                diags.push(SchemaError::UnknownProvider {
                    found: s.clone(),
                    supported: Provider::ALL
                        .iter()
                        .map(Provider::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                    span: (*span).into(),
                });
                Provider::Postgres
            }
        }
        Some(other) => {
            diags.push(config_error(
                "`provider` must be a quoted string",
                "e.g. `provider = \"postgres\"`",
                other.span(),
            ));
            Provider::Postgres
        }
        None => {
            diags.push(config_error(
                "`datasource` block has no `provider`",
                "add `provider = \"postgres\"`, `provider = \"sqlite\"`, or `provider = \"mysql\"`",
                block.span,
            ));
            Provider::Postgres
        }
    };

    let url = match block.get("url").map(|e| &e.value) {
        Some(Value::Env(var, _)) => DatasourceUrl::Env(var.clone()),
        Some(Value::Str(s, _)) => DatasourceUrl::Literal(s.clone()),
        Some(other) => {
            diags.push(config_error(
                "`url` must be a quoted string or `env(\"VAR\")`",
                "prefer `url = env(\"DATABASE_URL\")` so no credential is committed",
                other.span(),
            ));
            DatasourceUrl::Env("DATABASE_URL".to_owned())
        }
        None => {
            diags.push(config_error(
                "`datasource` block has no `url`",
                "add `url = env(\"DATABASE_URL\")`",
                block.span,
            ));
            DatasourceUrl::Env("DATABASE_URL".to_owned())
        }
    };

    Datasource {
        name: block.name.clone(),
        provider,
        url,
        span: block.span,
    }
}

fn lower_generator(ast: &Ast) -> Generator {
    let default = Generator::default();
    let Some(block) = ast.generators().next() else {
        return default;
    };

    let string_or = |key: &str, fallback: String| -> String {
        block
            .get(key)
            .and_then(|e| e.value.as_str())
            .map_or(fallback, str::to_owned)
    };

    Generator {
        name: block.name.clone(),
        output: string_or("output", default.output.clone()),
        module_name: string_or("module_name", default.module_name.clone()),
        max_include_depth: block
            .get("max_include_depth")
            .and_then(|e| match &e.value {
                Value::Num(n, _) => n.parse().ok(),
                _ => None,
            })
            .unwrap_or(default.max_include_depth),
        span: block.span,
    }
}

/// Stands in for a `datasource` block that is missing or unusable, so lowering
/// can continue and report the rest of the file's problems in the same run.
fn fallback_datasource() -> Datasource {
    Datasource {
        name: "db".to_owned(),
        provider: Provider::Postgres,
        url: DatasourceUrl::Env("DATABASE_URL".to_owned()),
        span: Span::EMPTY,
    }
}

fn missing_block(kind: &str, advice: &str) -> SchemaError {
    SchemaError::Syntax {
        message: format!("schema has no `{kind}` block"),
        advice: Some(advice.to_owned()),
        span: Span::new(0, 1).into(),
        context: format!("`{kind}` is required"),
    }
}

fn config_error(message: &str, advice: &str, span: Span) -> SchemaError {
    SchemaError::Syntax {
        message: message.to_owned(),
        advice: Some(advice.to_owned()),
        span: span.into(),
        context: "invalid configuration".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Pass 2a — enums
// ---------------------------------------------------------------------------

fn lower_enums(ast: &Ast, diags: &mut Diagnostics) -> IndexMap<EnumName, EnumDef> {
    let mut out = IndexMap::new();
    for decl in ast.enums() {
        out.insert(EnumName::new(&decl.name), lower_enum(decl, diags));
    }
    out
}

fn lower_enum(decl: &EnumDecl, diags: &mut Diagnostics) -> EnumDef {
    let mut variants: IndexMap<String, EnumVariant> = IndexMap::new();

    for v in &decl.variants {
        match variants.entry(v.name.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(EnumVariant {
                    name: v.name.clone(),
                    db_name: v.map.clone().unwrap_or_else(|| v.name.clone()),
                    docs: v.docs.clone(),
                    span: v.span,
                });
            }
            // V14
            Entry::Occupied(existing) => diags.push(SchemaError::DuplicateVariant {
                name: decl.name.clone(),
                variant: v.name.clone(),
                span: v.span.into(),
                first: existing.get().span.into(),
            }),
        }
    }

    EnumDef {
        name: EnumName::new(&decl.name),
        db_name: naming::enum_type_name(&decl.name),
        variants,
        docs: decl.docs.clone(),
        span: decl.span,
    }
}

// ---------------------------------------------------------------------------
// Pass 2b — models and fields
// ---------------------------------------------------------------------------

fn lower_models(
    ast: &Ast,
    env: &NameEnv,
    enums: &IndexMap<EnumName, EnumDef>,
    diags: &mut Diagnostics,
) -> IndexMap<ModelName, Model> {
    let mut out = IndexMap::new();
    for decl in ast.models() {
        let model = lower_model(decl, env, enums, diags);
        out.insert(model.name.clone(), model);
    }
    out
}

fn lower_model(
    decl: &ModelDecl,
    env: &NameEnv,
    enums: &IndexMap<EnumName, EnumDef>,
    diags: &mut Diagnostics,
) -> Model {
    let table = block_attr(decl, "map")
        .and_then(|a| a.first_positional().and_then(Value::as_str))
        .map_or_else(|| naming::table_name(&decl.name), str::to_owned);

    let mut fields: IndexMap<FieldName, Field> = IndexMap::new();
    for f in &decl.fields {
        let field = lower_field(f, env, enums, diags);
        match fields.entry(field.name.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(field);
            }
            // V04
            Entry::Occupied(existing) => diags.push(SchemaError::DuplicateField {
                model: decl.name.clone(),
                field: f.name.clone(),
                span: f.span.into(),
                first: existing.get().span.into(),
            }),
        }
    }

    let primary_key = lower_primary_key(decl, diags);
    let indexes = lower_indexes(decl, &table, &fields);
    let uniques = lower_uniques(decl, &table, &fields);

    Model {
        name: ModelName::new(&decl.name),
        table,
        fields,
        primary_key,
        indexes,
        uniques,
        docs: decl.docs.clone(),
        span: decl.span,
    }
}

fn block_attr<'a>(decl: &'a ModelDecl, path: &str) -> Option<&'a Attr> {
    decl.block_attrs.iter().find(|a| a.path == path)
}

fn lower_field(
    decl: &FieldDecl,
    env: &NameEnv,
    enums: &IndexMap<EnumName, EnumDef>,
    diags: &mut Diagnostics,
) -> Field {
    let base = base_kind(decl, env, diags);

    let kind = if decl.arity == Arity::List {
        FieldKind::List(Box::new(base))
    } else {
        base
    };

    let column = decl
        .attr("map")
        .and_then(|a| a.first_positional().and_then(Value::as_str))
        .map_or_else(|| naming::column_name(&decl.name), str::to_owned);

    let attrs = FieldAttrs {
        is_id: decl.has_attr("id"),
        is_unique: decl.has_attr("unique"),
        is_updated_at: decl.has_attr("updatedAt"),
        ignore: decl.has_attr("ignore"),
        native_type: native_type(decl),
        renamed_from: decl
            .attr("renamedFrom")
            .and_then(|a| a.first_positional().and_then(Value::as_str))
            .map(str::to_owned),
    };

    let default = decl
        .attr("default")
        .and_then(|a| lower_default(a, &kind, enums, diags));

    check_attribute_targets(decl, &kind, diags);

    Field {
        name: FieldName::new(&decl.name),
        column,
        kind,
        optional: decl.arity == Arity::Optional,
        default,
        attrs,
        docs: decl.docs.clone(),
        span: decl.span,
    }
}

/// Resolves the written type name to a scalar, an enum, or a relation (V02).
fn base_kind(decl: &FieldDecl, env: &NameEnv, diags: &mut Diagnostics) -> FieldKind {
    if let Some(scalar) = ScalarType::parse(&decl.type_name) {
        return FieldKind::Scalar(scalar);
    }
    match env.kind_of(&decl.type_name) {
        Some(DeclKind::Enum) => FieldKind::Enum(EnumName::new(&decl.type_name)),
        Some(DeclKind::Model) => FieldKind::Relation(relation_ref(decl)),
        None => {
            let candidates = env.type_candidates();
            let advice = suggest::closest(&decl.type_name, candidates.iter())
                .map(|c| format!("did you mean `{c}`?"))
                .or_else(|| {
                    Some(format!(
                        "declare `model {0}` or `enum {0}`, or use a built-in scalar",
                        decl.type_name
                    ))
                });
            diags.push(SchemaError::UnknownType {
                found: decl.type_name.clone(),
                advice,
                span: decl.type_span.into(),
            });
            // Substituting a scalar keeps the rest of the model lowerable, so the
            // author sees every other problem in the same run.
            FieldKind::Scalar(ScalarType::String)
        }
    }
}

fn relation_ref(decl: &FieldDecl) -> RelationRef {
    let attr = decl.attr("relation");
    let name = attr.and_then(|a| {
        a.first_positional()
            .and_then(Value::as_str)
            .or_else(|| a.named("name").and_then(Value::as_str))
            .map(str::to_owned)
    });

    let through = attr
        .and_then(|a| a.named("through"))
        .and_then(Value::as_ident);

    let name_list = |key: &str| -> Vec<FieldName> {
        attr.and_then(|a| a.named(key))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_ident().map(FieldName::new))
                    .collect()
            })
            .unwrap_or_default()
    };

    let action = |key: &str| -> Option<ReferentialAction> {
        attr.and_then(|a| a.named(key))
            .and_then(Value::as_ident)
            .and_then(ReferentialAction::parse)
    };

    RelationRef {
        target: ModelName::new(&decl.type_name),
        name,
        through: through.map(ModelName::new),
        fields: name_list("fields"),
        references: name_list("references"),
        on_delete: action("onDelete"),
        on_update: action("onUpdate"),
        resolved: None,
        span: attr.map_or(decl.span, |a| a.span),
    }
}

fn native_type(decl: &FieldDecl) -> Option<NativeType> {
    let attr = decl
        .attrs
        .iter()
        .find(|a| a.path.starts_with("db.") && a.path.len() > 3)?;
    Some(NativeType {
        name: attr.path["db.".len()..].to_owned(),
        args: attr.positional().map(Value::describe).collect(),
        span: attr.span,
    })
}

/// V10 — attributes that only make sense on some kinds of field.
fn check_attribute_targets(decl: &FieldDecl, kind: &FieldKind, diags: &mut Diagnostics) {
    let described = describe_kind(kind);

    if decl.has_attr("updatedAt") && !matches!(kind, FieldKind::Scalar(ScalarType::DateTime)) {
        diags.push(SchemaError::InvalidAttributeTarget {
            attribute: "updatedAt".to_owned(),
            found: described.clone(),
            advice: Some("`@updatedAt` stamps a timestamp, so it needs a `DateTime` field".into()),
            span: decl.attr("updatedAt").map_or(decl.span, |a| a.span).into(),
        });
    }

    if decl.has_attr("id") && matches!(kind, FieldKind::List(_) | FieldKind::Relation(_)) {
        diags.push(SchemaError::InvalidAttributeTarget {
            attribute: "id".to_owned(),
            found: described.clone(),
            advice: Some(
                "mark the foreign key column with `@id`, not the navigation property".into(),
            ),
            span: decl.attr("id").map_or(decl.span, |a| a.span).into(),
        });
    }

    if let Some(attr) = decl.attrs.iter().find(|a| a.path.starts_with("db.")) {
        if matches!(kind, FieldKind::List(_) | FieldKind::Relation(_)) {
            diags.push(SchemaError::InvalidAttributeTarget {
                attribute: attr.path.clone(),
                found: described,
                advice: Some(
                    "native database types apply to columns; put it on the foreign key field"
                        .into(),
                ),
                span: attr.span.into(),
            });
        }
    }
}

fn describe_kind(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Scalar(s) => s.to_string(),
        FieldKind::Enum(e) => e.to_string(),
        FieldKind::Relation(r) => r.target.to_string(),
        FieldKind::List(inner) => format!("{}[]", describe_kind(inner)),
    }
}

// ---------------------------------------------------------------------------
// Defaults (V09)
// ---------------------------------------------------------------------------

fn lower_default(
    attr: &Attr,
    kind: &FieldKind,
    enums: &IndexMap<EnumName, EnumDef>,
    diags: &mut Diagnostics,
) -> Option<DefaultValue> {
    let value = attr.first_positional()?;
    let expected = describe_kind(kind);

    let mismatch = |advice: String, diags: &mut Diagnostics| {
        diags.push(SchemaError::DefaultTypeMismatch {
            expected: expected.clone(),
            advice: Some(advice),
            span: value.span().into(),
        });
    };

    let lowered = match value {
        Value::Func { name, args, .. } if name == "dbgenerated" => {
            let sql = args.first().and_then(Value::as_str).unwrap_or_default();
            return Some(DefaultValue::DbGenerated(sql.to_owned()));
        }
        Value::Func { name, .. } => {
            if let Some(f) = DefaultFn::parse(name) {
                DefaultValue::Function(f)
            } else {
                mismatch(
                    format!(
                        "unknown default function; supported: {}",
                        DefaultFn::ALL
                            .iter()
                            .map(|f| format!("{}()", f.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    diags,
                );
                return None;
            }
        }
        Value::Str(s, _) => DefaultValue::Literal(Literal::String(s.clone())),
        Value::Bool(b, _) => DefaultValue::Literal(Literal::Bool(*b)),
        Value::Num(n, _) => {
            if n.contains('.') {
                DefaultValue::Literal(Literal::Float(n.parse().unwrap_or_default()))
            } else {
                DefaultValue::Literal(Literal::Int(n.parse().unwrap_or_default()))
            }
        }
        Value::Ident(i, _) => DefaultValue::Literal(Literal::EnumVariant(i.clone())),
        Value::Array(..) | Value::Env(..) => {
            mismatch("defaults are literals or functions".to_owned(), diags);
            return None;
        }
    };

    if let Err(advice) = default_matches(kind, &lowered, enums) {
        mismatch(advice, diags);
    }
    Some(lowered)
}

/// Whether a lowered default is usable on a field of this kind (V09).
fn default_matches(
    kind: &FieldKind,
    value: &DefaultValue,
    enums: &IndexMap<EnumName, EnumDef>,
) -> Result<(), String> {
    use ScalarType as S;

    let scalar = match kind {
        FieldKind::Scalar(s) => *s,
        FieldKind::Enum(name) => {
            let DefaultValue::Literal(Literal::EnumVariant(variant)) = value else {
                return Err(format!("use one of the variants of `{name}`"));
            };
            let Some(def) = enums.get(name.as_str()) else {
                return Ok(()); // the enum itself is already reported as unknown
            };
            return if def.variants.contains_key(variant) {
                Ok(())
            } else {
                Err(format!(
                    "`{name}` has no variant `{variant}`; known variants: {}",
                    def.variants.keys().cloned().collect::<Vec<_>>().join(", ")
                ))
            };
        }
        FieldKind::List(_) | FieldKind::Relation(_) => {
            return Err("relations cannot have a default; set the foreign key instead".to_owned());
        }
    };

    match value {
        DefaultValue::DbGenerated(_) => Ok(()),
        DefaultValue::Function(f) => match (f, scalar) {
            (DefaultFn::Now, S::DateTime | S::Date | S::Time)
            | (DefaultFn::Uuid4 | DefaultFn::Uuid7, S::Uuid | S::String)
            | (DefaultFn::Cuid2 | DefaultFn::Nanoid, S::String)
            | (DefaultFn::AutoIncrement, S::Int | S::BigInt) => Ok(()),
            _ => Err(format!("`{}()` cannot produce a `{scalar}`", f.as_str())),
        },
        DefaultValue::Literal(lit) => match (lit, scalar) {
            (
                Literal::String(_),
                S::String | S::Uuid | S::Json | S::DateTime | S::Date | S::Time,
            )
            | (Literal::Int(_), S::Int | S::BigInt | S::Float | S::Decimal)
            | (Literal::Float(_), S::Float | S::Decimal)
            | (Literal::Bool(_), S::Boolean) => Ok(()),
            (Literal::EnumVariant(v), _) => Err(format!(
                "`{v}` is not a `{scalar}` literal; quote it if you meant a string"
            )),
            _ => Err(format!("this literal is not a valid `{scalar}`")),
        },
    }
}

// ---------------------------------------------------------------------------
// Keys and indexes
// ---------------------------------------------------------------------------

fn lower_primary_key(decl: &ModelDecl, diags: &mut Diagnostics) -> PrimaryKey {
    let inline: Vec<&FieldDecl> = decl.fields.iter().filter(|f| f.has_attr("id")).collect();
    let block = block_attr(decl, "id");

    // V01 — more than one declaration of the key.
    if inline.len() > 1 {
        diags.push(SchemaError::MultiplePrimaryKeys {
            model: decl.name.clone(),
            span: inline[1].span.into(),
            first: inline[0].span.into(),
        });
    }
    if let (Some(block), Some(first)) = (block, inline.first()) {
        diags.push(SchemaError::MultiplePrimaryKeys {
            model: decl.name.clone(),
            span: block.span.into(),
            first: first.span.into(),
        });
    }

    if let Some(block) = block {
        return PrimaryKey {
            fields: field_list(block),
            name: block
                .named("map")
                .and_then(Value::as_str)
                .map(str::to_owned),
            span: block.span,
        };
    }

    match inline.first() {
        Some(f) => PrimaryKey {
            fields: vec![FieldName::new(&f.name)],
            name: None,
            span: f.span,
        },
        // Left empty on purpose: `validate` reports V01 against the finished IR,
        // so a model with no key still lowers and its other problems are found.
        None => PrimaryKey {
            fields: Vec::new(),
            name: None,
            span: decl.name_span,
        },
    }
}

/// The `[a, b]` first argument of a block attribute.
fn field_list(attr: &Attr) -> Vec<FieldName> {
    attr.first_positional()
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_ident().map(FieldName::new))
                .collect()
        })
        .unwrap_or_default()
}

/// Column names for a list of fields, falling back to the field name when the
/// field does not exist — V11 reports that separately, and a placeholder keeps
/// the derived constraint name readable in the meantime.
fn columns_of(fields: &[FieldName], model_fields: &IndexMap<FieldName, Field>) -> Vec<String> {
    fields
        .iter()
        .map(|f| {
            model_fields
                .get(f.as_str())
                .map_or_else(|| f.to_string(), |field| field.column.clone())
        })
        .collect()
}

fn lower_indexes(
    decl: &ModelDecl,
    table: &str,
    fields: &IndexMap<FieldName, Field>,
) -> Vec<IndexDef> {
    decl.block_attrs
        .iter()
        .filter(|a| a.path == "index")
        .map(|attr| {
            let names = field_list(attr);
            let columns = columns_of(&names, fields);
            IndexDef {
                db_name: attr
                    .named("map")
                    .and_then(Value::as_str)
                    .map_or_else(|| naming::index_name(table, &columns), str::to_owned),
                fields: names
                    .into_iter()
                    .map(|field| IndexField {
                        field,
                        order: SortOrder::Asc,
                    })
                    .collect(),
                span: attr.span,
            }
        })
        .collect()
}

fn lower_uniques(
    decl: &ModelDecl,
    table: &str,
    fields: &IndexMap<FieldName, Field>,
) -> Vec<UniqueDef> {
    decl.block_attrs
        .iter()
        .filter(|a| a.path == "unique")
        .map(|attr| {
            let names = field_list(attr);
            let columns = columns_of(&names, fields);
            UniqueDef {
                db_name: attr
                    .named("map")
                    .and_then(Value::as_str)
                    .map_or_else(|| naming::unique_name(table, &columns), str::to_owned),
                fields: names,
                span: attr.span,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Pass 3 — relations (V05–V08, V13)
// ---------------------------------------------------------------------------

/// One end of a relation as written, before the two ends are paired.
struct Side {
    model: ModelName,
    field: FieldName,
    target: ModelName,
    rel_name: Option<String>,
    /// Many-to-many join model this end belongs to, either as `through:` on a
    /// list field or as the join model itself for its foreign-key relations.
    join_model: Option<ModelName>,
    is_owner: bool,
    is_list: bool,
    optional: bool,
    span: Span,
}

fn resolve_relations(
    models: &mut IndexMap<ModelName, Model>,
    diags: &mut Diagnostics,
) -> Vec<ResolvedRelation> {
    let sides = collect_sides(models, diags);

    let (through_sides, regular_sides): (Vec<_>, Vec<_>) =
        sides.into_iter().partition(|s| s.join_model.is_some());

    // Grouping key: the explicit relation name if there is one, plus the unordered
    // pair of models. Without the name, two relations between the same pair land
    // in one group — which is exactly the ambiguity V08 exists to reject.
    let mut groups: IndexMap<(Option<String>, String, String), Vec<Side>> = IndexMap::new();
    for side in regular_sides {
        let (a, b) = {
            let (x, y) = (side.model.to_string(), side.target.to_string());
            if x <= y { (x, y) } else { (y, x) }
        };
        groups
            .entry((side.rel_name.clone(), a, b))
            .or_default()
            .push(side);
    }

    let mut relations = Vec::new();
    for group in groups.into_values() {
        resolve_group(&group, models, &mut relations, diags);
    }

    resolve_through_relations(through_sides, models, &mut relations, diags);
    relations
}

fn is_join_fk_side(model: &Model, target: &Model, rel: &RelationRef) -> bool {
    rel.through.is_none()
        && !rel.fields.is_empty()
        && target.fields.values().any(|f| {
            f.relation()
                .is_some_and(|r| r.through.as_ref() == Some(&model.name))
        })
}

fn collect_sides(models: &IndexMap<ModelName, Model>, diags: &mut Diagnostics) -> Vec<Side> {
    let mut sides = Vec::new();
    let mut poisoned: std::collections::HashSet<ModelName> = std::collections::HashSet::new();
    for model in models.values() {
        for field in model.fields.values() {
            let Some(rel) = field.relation() else {
                continue;
            };

            if rel.through.is_some() && !field.is_list() {
                if let Some(t) = &rel.through {
                    poisoned.insert(t.clone());
                }
                diags.push(SchemaError::ThroughOnNonList {
                    model: model.name.to_string(),
                    field: field.name.to_string(),
                    advice: Some("use a list field like `tags Tag[]`".to_owned()),
                    span: rel.span.into(),
                });
                continue;
            }

            if rel.through.is_some() && !rel.fields.is_empty() {
                if let Some(t) = &rel.through {
                    poisoned.insert(t.clone());
                }
                diags.push(SchemaError::ThroughWithFields {
                    model: model.name.to_string(),
                    field: field.name.to_string(),
                    advice: Some("`through` relations do not need `fields:`".to_owned()),
                    span: rel.span.into(),
                });
                continue;
            }

            let join_model = if let Some(t) = rel.through.clone() {
                Some(t)
            } else if let Some(target) = models.get(rel.target.as_str()) {
                if is_join_fk_side(model, target, rel) {
                    Some(model.name.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if join_model.as_ref().is_some_and(|m| poisoned.contains(m)) {
                continue;
            }

            sides.push(Side {
                model: model.name.clone(),
                field: field.name.clone(),
                target: rel.target.clone(),
                rel_name: rel.name.clone(),
                join_model,
                is_owner: !rel.fields.is_empty() && rel.through.is_none(),
                is_list: field.is_list(),
                optional: field.optional,
                span: rel.span,
            });
        }
    }
    sides.retain(|s| s.join_model.as_ref().is_none_or(|m| !poisoned.contains(m)));
    sides
}

fn resolve_group(
    group: &[Side],
    models: &mut IndexMap<ModelName, Model>,
    relations: &mut Vec<ResolvedRelation>,
    diags: &mut Diagnostics,
) {
    let owner_positions: Vec<usize> = (0..group.len()).filter(|i| group[*i].is_owner).collect();
    let owners: Vec<&Side> = owner_positions.iter().map(|i| &group[*i]).collect();

    // V08 — more than two ends means two unnamed relations were conflated. Point
    // at the two foreign-key sides where there are two: those are the lines the
    // author has to name, and the list side is only along for the ride.
    if group.len() > 2 {
        let (first, second) = if owners.len() >= 2 {
            (owners[0], owners[1])
        } else {
            (&group[0], &group[1])
        };
        diags.push(SchemaError::AmbiguousRelation {
            model: second.model.to_string(),
            target: second.target.to_string(),
            span: second.span.into(),
            first: first.span.into(),
        });
        return;
    }

    if owners.len() > 1 {
        // Both ends claim the foreign key; naming them is how the author says
        // which relation each end belongs to.
        diags.push(SchemaError::AmbiguousRelation {
            model: owners[1].model.to_string(),
            target: owners[1].target.to_string(),
            span: owners[1].span.into(),
            first: owners[0].span.into(),
        });
        return;
    }

    let Some(&owner_index) = owner_positions.first() else {
        // V08 — nobody declared `fields:`.
        let side = &group[0];
        diags.push(SchemaError::MissingRelationOwner {
            model: side.model.to_string(),
            target: side.target.to_string(),
            span: side.span.into(),
        });
        return;
    };
    let owner = &group[owner_index];

    let back = group
        .iter()
        .enumerate()
        .find(|(i, _)| *i != owner_index)
        .map(|(_, s)| s);
    if back.is_none() {
        // V08 — the target never declared the other end.
        diags.push(SchemaError::MissingBackRelation {
            model: owner.model.to_string(),
            field: owner.field.to_string(),
            target: owner.target.to_string(),
            back_name: naming::pluralize(&naming::snake_case(owner.model.as_str())),
            span: owner.span.into(),
        });
        return;
    }

    if let Some(resolved) = build_relation(owner, back, models, diags) {
        let index = relations.len();
        relations.push(resolved);
        mark_resolved(models, &owner.model, &owner.field, index);
        if let Some(b) = back {
            mark_resolved(models, &b.model, &b.field, index);
        }
    }
}

fn mark_resolved(
    models: &mut IndexMap<ModelName, Model>,
    model: &ModelName,
    field: &FieldName,
    index: usize,
) {
    if let Some(rel) = models
        .get_mut(model.as_str())
        .and_then(|m| m.fields.get_mut(field.as_str()))
        .and_then(relation_mut)
    {
        rel.resolved = Some(index);
    }
}

fn relation_mut(field: &mut Field) -> Option<&mut RelationRef> {
    match &mut field.kind {
        FieldKind::Relation(r) => Some(r),
        FieldKind::List(inner) => match inner.as_mut() {
            FieldKind::Relation(r) => Some(r),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_through_relations(
    sides: Vec<Side>,
    models: &mut IndexMap<ModelName, Model>,
    relations: &mut Vec<ResolvedRelation>,
    diags: &mut Diagnostics,
) {
    // Group by (relation_name, join_model). A valid many-to-many group has four
    // ends: the two list sides on the endpoints and the two FK-owner sides in
    // the join model.
    let mut groups: IndexMap<(Option<String>, ModelName), Vec<Side>> = IndexMap::new();
    for side in sides {
        groups
            .entry((side.rel_name.clone(), side.join_model.clone().unwrap()))
            .or_default()
            .push(side);
    }

    for group in groups.into_values() {
        if group.len() != 4 {
            diags.push(SchemaError::InvalidJoinModel {
                through: group[0].join_model.as_ref().unwrap().to_string(),
                owner: group[0].model.to_string(),
                target: group[0].target.to_string(),
                advice: Some("a many-to-many through model needs two endpoint list fields and two join foreign keys".to_owned()),
                span: group[0].span.into(),
            });
            continue;
        }

        let lists: Vec<&Side> = group.iter().filter(|s| !s.is_owner).collect();
        let fks: Vec<&Side> = group.iter().filter(|s| s.is_owner).collect();

        if lists.len() != 2 || fks.len() != 2 {
            diags.push(SchemaError::InvalidJoinModel {
                through: group[0].join_model.as_ref().unwrap().to_string(),
                owner: group[0].model.to_string(),
                target: group[0].target.to_string(),
                advice: Some(
                    "a many-to-many needs exactly two list sides and two FK sides".to_owned(),
                ),
                span: group[0].span.into(),
            });
            continue;
        }

        let a = lists[0];
        let b = lists[1];
        if a.model != b.target || b.model != a.target {
            diags.push(SchemaError::MissingBackRelation {
                model: a.model.to_string(),
                field: a.field.to_string(),
                target: a.target.to_string(),
                back_name: format!("{}[]", a.target),
                span: a.span.into(),
            });
            continue;
        }

        // Build the join model's two ManyToOne relations first. The back side
        // for each is the list field that lives on the targeted endpoint model.
        for fk in &fks {
            let back = lists.iter().copied().find(|s| s.model == fk.target);
            if let Some(resolved) = build_relation(fk, back, models, diags) {
                let index = relations.len();
                relations.push(resolved);
                mark_resolved(models, &fk.model, &fk.field, index);
            }
        }

        // A many-to-many is a pair of list fields that point at each other.
        // Each endpoint needs its own `ResolvedRelation` so the owner/target
        // orientation matches the field being resolved.
        if let Some(resolved) = build_many_to_many_relation(a, b, &fks, models, diags) {
            let index = relations.len();
            relations.push(resolved);
            mark_resolved(models, &a.model, &a.field, index);
        }
        if let Some(resolved) = build_many_to_many_relation(b, a, &fks, models, diags) {
            let index = relations.len();
            relations.push(resolved);
            mark_resolved(models, &b.model, &b.field, index);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn build_relation(
    owner: &Side,
    back: Option<&Side>,
    models: &IndexMap<ModelName, Model>,
    diags: &mut Diagnostics,
) -> Option<ResolvedRelation> {
    let owner_model = models.get(owner.model.as_str())?;
    let owner_field = owner_model.fields.get(owner.field.as_str())?;
    let rel = owner_field.relation()?;
    let target_model = models.get(owner.target.as_str())?;

    // V05 — every `fields:` entry must be a column-bearing field of this model.
    let mut fk_fields = Vec::new();
    for name in &rel.fields {
        match owner_model.fields.get(name.as_str()) {
            Some(f) if f.has_column() && f.relation().is_none() => fk_fields.push(f),
            _ => {
                diags.push(SchemaError::UnknownRelationField {
                    model: owner.model.to_string(),
                    field: owner.field.to_string(),
                    missing: name.to_string(),
                    span: rel.span.into(),
                });
                return None;
            }
        }
    }

    // `references:` defaults to the target's primary key, which is what it is in
    // every schema that bothers to write it out.
    let reference_names: Vec<FieldName> = if rel.references.is_empty() {
        target_model.primary_key.fields.clone()
    } else {
        rel.references.clone()
    };

    // V06 — references must exist and be unique or the primary key.
    let mut ref_fields = Vec::new();
    for name in &reference_names {
        let Some(f) = target_model.fields.get(name.as_str()) else {
            diags.push(SchemaError::InvalidRelationTarget {
                model: owner.model.to_string(),
                field: owner.field.to_string(),
                target: owner.target.to_string(),
                missing: name.to_string(),
                span: rel.span.into(),
            });
            return None;
        };
        let is_key = target_model.primary_key.fields.contains(name);
        let is_unique = f.attrs.is_unique
            || target_model
                .uniques
                .iter()
                .any(|u| u.fields.len() == 1 && u.fields[0] == *name);
        if !is_key && !is_unique {
            diags.push(SchemaError::InvalidRelationTarget {
                model: owner.model.to_string(),
                field: owner.field.to_string(),
                target: owner.target.to_string(),
                missing: name.to_string(),
                span: rel.span.into(),
            });
            return None;
        }
        ref_fields.push(f);
    }

    if fk_fields.len() != ref_fields.len() {
        diags.push(SchemaError::InvalidRelationTarget {
            model: owner.model.to_string(),
            field: owner.field.to_string(),
            target: owner.target.to_string(),
            missing: format!(
                "{} reference(s) for {} foreign key field(s)",
                ref_fields.len(),
                fk_fields.len()
            ),
            span: rel.span.into(),
        });
        return None;
    }

    // V07 — the join only works if the two columns hold the same thing.
    for (fk, target) in fk_fields.iter().zip(&ref_fields) {
        if fk.kind != target.kind {
            diags.push(SchemaError::RelationTypeMismatch {
                model: owner.model.to_string(),
                field: fk.name.to_string(),
                found: describe_kind(&fk.kind),
                expected: describe_kind(&target.kind),
                span: fk.span.into(),
            });
        }
    }

    // V13 — an optional relation needs a nullable foreign key.
    if owner.optional {
        for fk in &fk_fields {
            if !fk.optional {
                diags.push(SchemaError::RelationNullabilityMismatch {
                    model: owner.model.to_string(),
                    field: owner.field.to_string(),
                    fk: fk.name.to_string(),
                    fk_type: describe_kind(&fk.kind),
                    span: fk.span.into(),
                });
            }
        }
    }

    let owner_cols: Vec<String> = fk_fields.iter().map(|f| f.column.clone()).collect();
    let target_cols: Vec<String> = ref_fields.iter().map(|f| f.column.clone()).collect();

    // A back-reference that is a list means many owner rows per target row.
    let kind = if back.is_some_and(|b| b.is_list) {
        RelationKind::ManyToOne
    } else {
        RelationKind::OneToOne
    };

    let target_table = target_model.table.clone();

    Some(ResolvedRelation {
        name: rel
            .name
            .clone()
            .unwrap_or_else(|| format!("{}To{}", owner.model, owner.target)),
        kind,
        owner: owner.model.clone(),
        owner_cols: owner_cols.clone(),
        owner_field: owner.field.clone(),
        target: owner.target.clone(),
        target_table,
        target_cols,
        target_field: back.map(|b| b.field.clone()),
        // Prisma's defaults, and the right ones: deleting a row out from under a
        // required foreign key must fail, while an optional one can simply be
        // cleared.
        on_delete: rel.on_delete.unwrap_or(if owner.optional {
            ReferentialAction::SetNull
        } else {
            ReferentialAction::Restrict
        }),
        on_update: rel.on_update.unwrap_or(ReferentialAction::Cascade),
        optional: owner.optional,
        constraint_name: naming::foreign_key_name(&owner_model.table, &owner_cols),
        span: rel.span,
        join_model: None,
        join_owner_field: None,
        join_target_field: None,
    })
}

fn build_many_to_many_relation(
    owner: &Side,
    back: &Side,
    fks: &[&Side],
    models: &IndexMap<ModelName, Model>,
    diags: &mut Diagnostics,
) -> Option<ResolvedRelation> {
    let through_name = owner.join_model.as_ref()?;
    let through_model = models.get(through_name.as_str())?;

    let owner_fk = fks.iter().copied().find(|s| s.target == owner.model);
    let target_fk = fks.iter().copied().find(|s| s.target == back.model);

    let Some(owner_fk) = owner_fk else {
        diags.push(SchemaError::InvalidJoinModel {
            through: through_name.to_string(),
            owner: owner.model.to_string(),
            target: back.model.to_string(),
            advice: Some(format!(
                "`{through_name}` needs a foreign key to `{}`",
                owner.model
            )),
            span: owner.span.into(),
        });
        return None;
    };

    let Some(target_fk) = target_fk else {
        diags.push(SchemaError::InvalidJoinModel {
            through: through_name.to_string(),
            owner: owner.model.to_string(),
            target: back.model.to_string(),
            advice: Some(format!(
                "`{through_name}` needs a foreign key to `{}`",
                back.model
            )),
            span: back.span.into(),
        });
        return None;
    };

    let owner_fk_field = through_model.fields.get(owner_fk.field.as_str())?;
    let target_fk_field = through_model.fields.get(target_fk.field.as_str())?;
    let owner_fk_rel = owner_fk_field.relation()?;
    let target_fk_rel = target_fk_field.relation()?;

    let owner_cols: Vec<String> = owner_fk_rel
        .fields
        .iter()
        .filter_map(|n| {
            through_model
                .fields
                .get(n.as_str())
                .map(|f| f.column.clone())
        })
        .collect();
    let target_cols: Vec<String> = target_fk_rel
        .fields
        .iter()
        .filter_map(|n| {
            through_model
                .fields
                .get(n.as_str())
                .map(|f| f.column.clone())
        })
        .collect();

    Some(ResolvedRelation {
        name: owner
            .rel_name
            .clone()
            .unwrap_or_else(|| format!("{}To{}Via{}", owner.model, back.model, through_name)),
        kind: RelationKind::ManyToMany,
        owner: owner.model.clone(),
        owner_cols,
        owner_field: owner.field.clone(),
        target: back.model.clone(),
        target_table: through_model.table.clone(),
        target_cols,
        target_field: Some(back.field.clone()),
        on_delete: owner_fk_rel
            .on_delete
            .unwrap_or(ReferentialAction::Restrict),
        on_update: owner_fk_rel.on_update.unwrap_or(ReferentialAction::Cascade),
        optional: false,
        constraint_name: naming::foreign_key_name(&through_model.table, &[]),
        span: owner.span,
        join_model: Some(through_name.clone()),
        join_owner_field: Some(owner_fk.field.clone()),
        join_target_field: Some(target_fk.field.clone()),
    })
}
