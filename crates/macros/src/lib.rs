//! Proc-macro support for the ruprizzle runtime.
//!
//! Implemented alongside P4; see
//! `ProjectPlan/ImplementationPlan/ImplPlan05QueryBuilderRuntime.md`.
//!
//! This crate stays deliberately thin. The ORM's type safety comes from
//! generated column tokens (ADR-005), not from macro magic, so the only macros
//! here are conveniences such as the injection-safe `raw!` fragment builder.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

use std::collections::HashMap;
use std::{env, fs};

use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::quote;
use syn::{Expr, Token, parse_macro_input, punctuated::Punctuated};

/// Resolves the path prefix to the `ruprizzle` crate at macro-expansion time.
///
/// Inside the `ruprizzle` crate itself (e.g. its own unit or integration tests)
/// this returns `crate`. For downstream crates it returns the crate name as
/// declared in the user's `Cargo.toml` (commonly `::ruprizzle`).
fn crate_path() -> Result<proc_macro2::TokenStream, syn::Error> {
    match proc_macro_crate::crate_name("ruprizzle") {
        Ok(FoundCrate::Itself) => Ok(quote!(crate)),
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            Ok(quote!(::#ident))
        }
        Err(err) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("raw! requires the `ruprizzle` crate to be present in Cargo.toml: {err}"),
        )),
    }
}

/// Builds an injection-safe raw SQL fragment.
///
/// The first argument is a string literal containing `{}` placeholders. Each
/// remaining argument is an expression whose value is bound as a SQL parameter.
/// The placeholders are replaced with bind markers; the actual values are never
/// interpolated into the SQL string.
#[proc_macro]
pub fn raw(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input with Punctuated::<Expr, Token![,]>::parse_terminated);

    let mut iter = args.into_iter();
    let Some(first) = iter.next() else {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "raw! requires at least a format string",
        )
        .to_compile_error()
        .into();
    };

    let Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(format_str),
        ..
    }) = &first
    else {
        return syn::Error::new_spanned(
            first,
            "the first argument to raw! must be a string literal",
        )
        .to_compile_error()
        .into();
    };

    let values: Vec<Expr> = iter.collect();
    let literal = format_str.value();
    let placeholder_count = literal.matches("{}").count();

    if placeholder_count != values.len() {
        return syn::Error::new_spanned(
            format_str,
            format!(
                "raw! format string has {placeholder_count} placeholders but {} expressions were provided",
                values.len()
            ),
        )
        .to_compile_error()
        .into();
    }

    if let Some(err) = offline_schema_check(&literal, format_str) {
        return err.to_compile_error().into();
    }

    let parts: Vec<String> = literal.split("{}").map(String::from).collect();

    let part_lits: Vec<proc_macro2::Literal> = parts
        .iter()
        .map(|part| proc_macro2::Literal::string(part))
        .collect();

    let crate_path = match crate_path() {
        Ok(path) => path,
        Err(e) => return e.to_compile_error().into(),
    };
    let binds_ident = proc_macro2::Ident::new("__rz_raw_binds", proc_macro2::Span::mixed_site());

    let push_binds: Vec<proc_macro2::TokenStream> = values
        .iter()
        .map(|expr| quote! { #binds_ident.push(#crate_path::Encodable::to_value(&#expr)); })
        .collect();

    let expanded = quote! {
        #crate_path::RawFragment::new(
            ::std::vec![#(::std::string::String::from(#part_lits)),*],
            {
                let mut #binds_ident = ::std::vec![];
                #(#push_binds)*
                #binds_ident
            },
        )
    };

    expanded.into()
}

fn offline_schema_check(sql: &str, format_str: &syn::LitStr) -> Option<syn::Error> {
    let Ok(path) = env::var("RUPRIZZLE_OFFLINE_SCHEMA") else {
        return None;
    };
    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return Some(syn::Error::new_spanned(
                format_str,
                format!("RUPRIZZLE_OFFLINE_SCHEMA points to unreadable file `{path}`: {e}"),
            ));
        }
    };

    let schema = match ruprizzle_parser::parse(&path, &source) {
        Ok(s) => s,
        Err(e) => {
            return Some(syn::Error::new_spanned(
                format_str,
                format!("failed to parse offline schema `{path}`: {e:?}"),
            ));
        }
    };

    let table_to_model: HashMap<&str, &ruprizzle_core::ir::Model> = schema
        .models
        .values()
        .map(|m| (m.table.as_str(), m))
        .collect();

    let tokens = tokenise_sql(sql);
    for (idx, token) in tokens.iter().enumerate() {
        if let Some(model) = table_to_model.get(token.as_str()) {
            if let Some(next) = tokens.get(idx + 1) {
                if next == "." {
                    if let Some(column) = tokens.get(idx + 2) {
                        if !model.fields.values().any(|f| f.column == *column) {
                            return Some(syn::Error::new_spanned(
                                format_str,
                                format!("unknown column `{column}` on table `{token}`"),
                            ));
                        }
                    }
                }
            }
            continue;
        }

        if idx > 0
            && tokens
                .get(idx - 1)
                .is_some_and(|prev| is_table_context(prev))
            && !is_sql_keyword(token)
            && !looks_like_string_or_param(token)
        {
            return Some(syn::Error::new_spanned(
                format_str,
                format!("unknown table `{token}`"),
            ));
        }
    }

    None
}

fn tokenise_sql(sql: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_quote = '\0';

    for c in sql.chars() {
        if in_string {
            current.push(c);
            if c == string_quote {
                in_string = false;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            in_string = true;
            string_quote = c;
            current.push(c);
            continue;
        }
        if c.is_alphanumeric() || c == '_' || c == '*' || c == '$' {
            current.push(c);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if c == '.' {
                tokens.push(".".to_owned());
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_table_context(prev: &str) -> bool {
    matches!(
        prev.to_ascii_uppercase().as_str(),
        "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE"
    )
}

fn is_sql_keyword(token: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "INSERT", "UPDATE", "DELETE", "JOIN",
        "INNER", "LEFT", "RIGHT", "FULL", "OUTER", "ON", "GROUP", "BY", "ORDER", "LIMIT", "OFFSET",
        "HAVING", "VALUES", "SET", "AS", "WITH", "UNION", "ALL", "DISTINCT", "IS", "NULL", "TRUE",
        "FALSE", "IN", "BETWEEN", "LIKE", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END",
    ];
    KEYWORDS.iter().any(|&k| k == token.to_ascii_uppercase())
}

fn looks_like_string_or_param(token: &str) -> bool {
    token.starts_with('\'') || token.starts_with('"') || token.starts_with('$')
}
