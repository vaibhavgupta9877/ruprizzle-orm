//! Pest grammar binding and the walk from parse pairs to [`crate::ast`].
//!
//! Nothing here interprets the schema. It only turns a successful parse into
//! typed nodes with spans; every judgement about meaning happens in
//! [`crate::lower`] and [`crate::validate`].

use pest::Parser as _;
use pest::iterators::Pair;
use ruprizzle_core::span::Span;

use crate::ast::{
    Arg, Arity, Ast, Attr, Block, ConfigEntry, Decl, EnumDecl, FieldDecl, ModelDecl, Value,
    VariantDecl,
};

/// The generated Pest parser for `schema.ruprizzle`.
#[derive(pest_derive::Parser)]
#[grammar = "schema.pest"]
pub struct SchemaParser;

/// Parses source text into the loose AST.
///
/// # Errors
///
/// Returns the raw Pest error; [`crate::errors`] turns it into a diagnostic.
pub fn parse_ast(source: &str) -> Result<Ast, Box<pest::error::Error<Rule>>> {
    let mut pairs = SchemaParser::parse(Rule::schema, source).map_err(Box::new)?;
    // Pest's `schema = { SOI ~ decl* ~ EOI }` is a single successful match,
    // so the top-level iterator always produces exactly one pair.
    let schema = pairs.next().expect("Rule::schema always yields one pair");

    let mut ast = Ast::default();
    for pair in schema.into_inner() {
        let decl = match pair.as_rule() {
            Rule::datasource => Decl::Datasource(block(pair)),
            Rule::generator => Decl::Generator(block(pair)),
            Rule::enum_def => Decl::Enum(enum_decl(pair)),
            Rule::model_def => Decl::Model(model_decl(pair)),
            Rule::EOI => continue,
            // `decl` is `{ datasource | generator | enum_def | model_def }`, and
            // only those four rules plus EOI can appear inside `schema`.
            other => unreachable!("unexpected top-level rule {other:?}"),
        };
        ast.decls.push(decl);
    }
    Ok(ast)
}

fn span_of(pair: &Pair<'_, Rule>) -> Span {
    let s = pair.as_span();
    Span::new(s.start(), s.end())
}

/// The children of a declaration, with keyword tokens removed.
///
/// Keywords are atomic rather than silent so that `model` cannot match the start
/// of `modelish` (a silent rule would have implicit whitespace inserted before
/// its boundary check). The cost is a pair per keyword, dropped here so the rest
/// of the walk sees only meaningful children.
fn members(pair: Pair<'_, Rule>) -> Walk<'_> {
    pair.into_inner()
        .filter(|p| {
            !matches!(
                p.as_rule(),
                Rule::kw_datasource
                    | Rule::kw_generator
                    | Rule::kw_enum
                    | Rule::kw_model
                    | Rule::kw_env
            )
        })
        .collect::<Vec<_>>()
        .into_iter()
        .peekable()
}

/// A rewindable walk over a declaration's children.
type Walk<'a> = std::iter::Peekable<std::vec::IntoIter<Pair<'a, Rule>>>;

/// Collects leading `///` lines into one rustdoc string.
///
/// Returns `None` rather than an empty string so the IR can distinguish
/// "documented with nothing" from "not documented", which matters when deciding
/// whether to emit a rustdoc block at all.
fn take_docs(pairs: &mut Walk<'_>) -> Option<String> {
    let mut lines = Vec::new();
    while pairs
        .peek()
        .is_some_and(|p| p.as_rule() == Rule::doc_comment)
    {
        // We only enter this loop when `peek` confirmed a `doc_comment`,
        // so `next` cannot return `None`.
        let comment = pairs.next().expect("peeked");
        let text = comment
            .into_inner()
            .find(|p| p.as_rule() == Rule::doc_text)
            .map(|p| p.as_str().trim_end().to_owned())
            .unwrap_or_default();
        lines.push(text);
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn block(pair: Pair<'_, Rule>) -> Block {
    let span = span_of(&pair);
    let mut inner = members(pair);
    // `datasource`/`generator` both have `~ ident ~ "{"`, so the first
    // non-keyword child is the identifier.
    let name_pair = inner.next().expect("block always has a name");
    let name = name_pair.as_str().to_owned();

    let entries = inner
        .map(|kv| {
            let span = span_of(&kv);
            let mut parts = kv.into_inner();
            // `config_kv = { ident ~ "=" ~ config_value }` guarantees two children.
            let key = parts
                .next()
                .expect("config_kv has a key")
                .as_str()
                .to_owned();
            let value = value(parts.next().expect("config_kv has a value"));
            ConfigEntry { key, value, span }
        })
        .collect();

    Block {
        name,
        entries,
        span,
    }
}

fn enum_decl(pair: Pair<'_, Rule>) -> EnumDecl {
    let span = span_of(&pair);
    let mut inner = members(pair);
    let docs = take_docs(&mut inner);

    // `enum_def = { ... ~ ident ~ "{" ... }` always provides a name.
    let name_pair = inner.next().expect("enum always has a name");
    let name_span = span_of(&name_pair);
    let name = name_pair.as_str().to_owned();

    let variants = inner.map(variant_decl).collect();

    EnumDecl {
        name,
        name_span,
        variants,
        docs,
        span,
    }
}

fn variant_decl(pair: Pair<'_, Rule>) -> VariantDecl {
    let span = span_of(&pair);
    let mut inner = members(pair);
    let docs = take_docs(&mut inner);

    // `enum_variant = { doc_comment* ~ ident ~ ... }` always provides a name.
    let name = inner
        .next()
        .expect("variant always has a name")
        .as_str()
        .to_owned();
    let map = inner.next().map(|p| unescape(inner_text(&p)));

    VariantDecl {
        name,
        map,
        docs,
        span,
    }
}

fn model_decl(pair: Pair<'_, Rule>) -> ModelDecl {
    let span = span_of(&pair);
    let mut inner = members(pair);
    let docs = take_docs(&mut inner);

    // `model_def = { ... ~ ident ~ "{" ... }` always provides a name.
    let name_pair = inner.next().expect("model always has a name");
    let name_span = span_of(&name_pair);
    let name = name_pair.as_str().to_owned();

    let mut fields = Vec::new();
    let mut block_attrs = Vec::new();
    for member in inner {
        match member.as_rule() {
            Rule::field => fields.push(field_decl(member)),
            Rule::block_attr => block_attrs.push(attr(member)),
            // `model_member = _{ block_attr | field }` is the only shape allowed.
            other => unreachable!("unexpected model member {other:?}"),
        }
    }

    ModelDecl {
        name,
        name_span,
        fields,
        block_attrs,
        docs,
        span,
    }
}

fn field_decl(pair: Pair<'_, Rule>) -> FieldDecl {
    let span = span_of(&pair);
    let mut inner = members(pair);
    let docs = take_docs(&mut inner);

    // `field = { ... ~ ident ~ field_type ~ ... }` always provides both.
    let name_pair = inner.next().expect("field always has a name");
    let name_span = span_of(&name_pair);
    let name = name_pair.as_str().to_owned();

    // `field_type` is atomic — it has to be, so that a missing type is reported
    // as a field type rather than as a bare identifier — so the arity marker is
    // read off the text rather than from a child pair.
    let type_pair = inner.next().expect("field always has a type");
    // `field_type = @{ ident ~ (list_marker | opt_marker)? }` means a type
    // pair is always present after the field name.
    let type_span = span_of(&type_pair);
    let written = type_pair.as_str();
    let (type_name, arity) = if let Some(base) = written.strip_suffix("[]") {
        (base, Arity::List)
    } else if let Some(base) = written.strip_suffix('?') {
        (base, Arity::Optional)
    } else {
        (written, Arity::Required)
    };
    let type_name = type_name.to_owned();

    let attrs = inner.map(attr).collect();

    FieldDecl {
        name,
        name_span,
        type_name,
        type_span,
        arity,
        attrs,
        docs,
        span,
    }
}

fn attr(pair: Pair<'_, Rule>) -> Attr {
    let span = span_of(&pair);
    let mut inner = pair.into_inner();
    // `field_attr`/`block_attr` both start with `attr_path`, so the first
    // child is always the path.
    let path = inner
        .next()
        .expect("attribute always has a path")
        .as_str()
        .to_owned();

    let args = inner
        .next()
        .map(|list| list.into_inner().map(arg).collect())
        .unwrap_or_default();

    Attr { path, args, span }
}

fn arg(pair: Pair<'_, Rule>) -> Arg {
    // `arg = { named_arg | value }` expands to exactly one child.
    let inner = pair.into_inner().next().expect("arg wraps one value");
    match inner.as_rule() {
        Rule::named_arg => {
            let span = span_of(&inner);
            let mut parts = inner.into_inner();
            // `named_arg = { ident ~ ":" ~ value }` guarantees two children.
            let name = parts
                .next()
                .expect("named argument has a name")
                .as_str()
                .to_owned();
            let value = value(parts.next().expect("named argument has a value"));
            Arg::Named { name, value, span }
        }
        _ => Arg::Positional(value(inner)),
    }
}

fn inner_text(pair: &Pair<'_, Rule>) -> String {
    pair.clone()
        .into_inner()
        .next()
        .map_or_else(|| pair.as_str().to_owned(), |p| p.as_str().to_owned())
}

fn value(pair: Pair<'_, Rule>) -> Value {
    let span = span_of(&pair);
    match pair.as_rule() {
        Rule::string => Value::Str(unescape(inner_text(&pair)), span),
        Rule::number => Value::Num(pair.as_str().to_owned(), span),
        Rule::boolean => Value::Bool(pair.as_str() == "true", span),
        Rule::ident => Value::Ident(pair.as_str().to_owned(), span),
        Rule::env_call => Value::Env(
            unescape(inner_text(
                // `env_call = { kw_env ~ "(" ~ string ~ ")" }` always contains a string.
                &pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::string)
                    .expect("env() takes a string"),
            )),
            span,
        ),
        Rule::func_call => {
            // `func_call = { ident ~ "(" ~ ... }` always starts with the name.
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .expect("call always has a name")
                .as_str()
                .to_owned();
            Value::Func {
                name,
                args: inner.map(value).collect(),
                span,
            }
        }
        Rule::array => Value::Array(pair.into_inner().map(value).collect(), span),
        // `value = _{ func_call | array | string | number | boolean | ident }`
        // covers every rule that can reach this match.
        other => unreachable!("unexpected value rule {other:?}"),
    }
}

/// Resolves the escape sequences the grammar allows inside a string literal.
fn unescape(raw: String) -> String {
    if !raw.contains('\\') {
        return raw;
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}
