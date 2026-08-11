//! The loose syntax tree, one step away from the grammar.
//!
//! This mirrors what was *written*, not what it *means*: no types are resolved,
//! no names are mapped, no relation has two sides yet. Lowering turns it into the
//! strict IR.
//!
//! The separation is not ceremony. Relation resolution needs the complete set of
//! models, which does not exist part-way through a parse, so an IR built directly
//! in the parse walk would be wrong for any schema that references a model before
//! declaring it — which is most of them.

use ruprizzle_core::span::Span;

/// A parsed schema file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ast {
    /// Declarations in source order.
    pub decls: Vec<Decl>,
}

impl Ast {
    /// The `datasource` blocks, in source order.
    pub fn datasources(&self) -> impl Iterator<Item = &Block> {
        self.decls.iter().filter_map(|d| match d {
            Decl::Datasource(b) => Some(b),
            _ => None,
        })
    }

    /// The `generator` blocks, in source order.
    pub fn generators(&self) -> impl Iterator<Item = &Block> {
        self.decls.iter().filter_map(|d| match d {
            Decl::Generator(b) => Some(b),
            _ => None,
        })
    }

    /// The `enum` declarations, in source order.
    pub fn enums(&self) -> impl Iterator<Item = &EnumDecl> {
        self.decls.iter().filter_map(|d| match d {
            Decl::Enum(e) => Some(e),
            _ => None,
        })
    }

    /// The `model` declarations, in source order.
    pub fn models(&self) -> impl Iterator<Item = &ModelDecl> {
        self.decls.iter().filter_map(|d| match d {
            Decl::Model(m) => Some(m),
            _ => None,
        })
    }
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    /// `datasource db { ... }`
    Datasource(Block),
    /// `generator client { ... }`
    Generator(Block),
    /// `enum Role { ... }`
    Enum(EnumDecl),
    /// `model User { ... }`
    Model(ModelDecl),
}

/// A `datasource` or `generator` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Block name as written, e.g. `db`.
    pub name: String,
    /// `key = value` entries, in source order.
    pub entries: Vec<ConfigEntry>,
    /// Source location of the whole block.
    pub span: Span,
}

impl Block {
    /// The entry with the given key, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ConfigEntry> {
        self.entries.iter().find(|e| e.key == key)
    }
}

/// One `key = value` line inside a block.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    /// Left-hand side.
    pub key: String,
    /// Right-hand side.
    pub value: Value,
    /// Source location of the entry.
    pub span: Span,
}

/// An `enum` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    /// Name as written.
    pub name: String,
    /// Source location of the name alone, for diagnostics that point at it.
    pub name_span: Span,
    /// Variants in source order.
    pub variants: Vec<VariantDecl>,
    /// Joined `///` lines.
    pub docs: Option<String>,
    /// Source location of the whole declaration.
    pub span: Span,
}

/// One variant of an [`EnumDecl`].
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDecl {
    /// Name as written.
    pub name: String,
    /// `@map("...")`, if given.
    pub map: Option<String>,
    /// Joined `///` lines.
    pub docs: Option<String>,
    /// Source location of the variant.
    pub span: Span,
}

/// A `model` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDecl {
    /// Name as written.
    pub name: String,
    /// Source location of the name alone.
    pub name_span: Span,
    /// Fields in source order.
    pub fields: Vec<FieldDecl>,
    /// `@@`-attributes in source order.
    pub block_attrs: Vec<Attr>,
    /// Joined `///` lines.
    pub docs: Option<String>,
    /// Source location of the whole declaration.
    pub span: Span,
}

/// A field within a [`ModelDecl`].
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    /// Name as written.
    pub name: String,
    /// Source location of the name alone.
    pub name_span: Span,
    /// Type name as written, before resolution.
    pub type_name: String,
    /// Source location of the type.
    pub type_span: Span,
    /// Whether the type carried `[]` or `?`.
    pub arity: Arity,
    /// `@`-attributes in source order.
    pub attrs: Vec<Attr>,
    /// Joined `///` lines.
    pub docs: Option<String>,
    /// Source location of the whole field.
    pub span: Span,
}

impl FieldDecl {
    /// The first attribute whose path matches, if any.
    #[must_use]
    pub fn attr(&self, path: &str) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.path == path)
    }

    /// Whether an attribute with the given path is present.
    #[must_use]
    pub fn has_attr(&self, path: &str) -> bool {
        self.attr(path).is_some()
    }
}

/// How many values a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    /// `T` — exactly one.
    Required,
    /// `T?` — zero or one.
    Optional,
    /// `T[]` — many.
    List,
}

/// An `@attr` or `@@attr`, with its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    /// Dotted path as written, e.g. `id`, `db.VarChar`, `relation`.
    pub path: String,
    /// Arguments, in source order.
    pub args: Vec<Arg>,
    /// Source location of the attribute.
    pub span: Span,
}

impl Attr {
    /// The named argument with this name, if given.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&Value> {
        self.args.iter().find_map(|a| match a {
            Arg::Named { name: n, value, .. } if n == name => Some(value),
            _ => None,
        })
    }

    /// Positional arguments, in source order.
    pub fn positional(&self) -> impl Iterator<Item = &Value> {
        self.args.iter().filter_map(|a| match a {
            Arg::Positional(v) => Some(v),
            Arg::Named { .. } => None,
        })
    }

    /// The first positional argument, if any.
    #[must_use]
    pub fn first_positional(&self) -> Option<&Value> {
        self.positional().next()
    }
}

/// One argument of an [`Attr`].
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// `name: value`
    Named {
        /// Argument name.
        name: String,
        /// Argument value.
        value: Value,
        /// Source location of the whole argument.
        span: Span,
    },
    /// A bare value.
    Positional(Value),
}

/// A value written in an attribute argument or a config entry.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `"text"`
    Str(String, Span),
    /// A number, kept as written so integer and float stay distinguishable.
    Num(String, Span),
    /// `true` / `false`
    Bool(bool, Span),
    /// A bare identifier, e.g. `USER` or `Cascade`.
    Ident(String, Span),
    /// `env("DATABASE_URL")`
    Env(String, Span),
    /// `uuid7()`, `dbgenerated("...")`
    Func {
        /// Function name.
        name: String,
        /// Arguments, in source order.
        args: Vec<Value>,
        /// Source location of the call.
        span: Span,
    },
    /// `[a, b]`
    Array(Vec<Value>, Span),
}

impl Value {
    /// Source location of the value.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Value::Str(_, s)
            | Value::Num(_, s)
            | Value::Bool(_, s)
            | Value::Ident(_, s)
            | Value::Env(_, s)
            | Value::Func { span: s, .. }
            | Value::Array(_, s) => *s,
        }
    }

    /// The string, if this is a string literal.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s, _) => Some(s),
            _ => None,
        }
    }

    /// The identifier, if this is a bare identifier.
    #[must_use]
    pub fn as_ident(&self) -> Option<&str> {
        match self {
            Value::Ident(s, _) => Some(s),
            _ => None,
        }
    }

    /// The elements, if this is an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(v, _) => Some(v),
            _ => None,
        }
    }

    /// How this value reads in a diagnostic.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Value::Str(s, _) => format!("\"{s}\""),
            Value::Num(n, _) => n.clone(),
            Value::Bool(b, _) => b.to_string(),
            Value::Ident(i, _) => i.clone(),
            Value::Env(v, _) => format!("env(\"{v}\")"),
            Value::Func { name, .. } => format!("{name}(…)"),
            Value::Array(..) => "[…]".to_owned(),
        }
    }
}
