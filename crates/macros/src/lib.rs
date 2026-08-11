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

use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::quote;
use syn::{Expr, Token, parse_macro_input, punctuated::Punctuated};

/// Resolves the path prefix to the `ruprizzle` crate at macro-expansion time.
///
/// Inside the `ruprizzle` crate itself (e.g. its own unit or integration tests)
/// this returns `crate`. For downstream crates it returns the crate name as
/// declared in the user's `Cargo.toml` (commonly `::ruprizzle`).
fn crate_path() -> proc_macro2::TokenStream {
    match proc_macro_crate::crate_name("ruprizzle") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#ident)
        }
        Err(err) => {
            panic!("raw! requires the `ruprizzle` crate to be present in Cargo.toml: {err}")
        }
    }
}

/// Builds an injection-safe raw SQL fragment.
///
/// The first argument is a string literal containing `{}` placeholders. Each
/// remaining argument is an expression whose value is bound as a SQL parameter.
/// The placeholders are replaced with bind markers; the actual values are never
/// interpolated into the SQL string.
///
/// # Example
///
/// ```ignore
/// use ruprizzle::raw;
///
/// let fragment = raw!("email = {}", "user@example.com");
/// assert_eq!(fragment.sql(), "email = $1");
/// ```
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

    let parts: Vec<String> = literal.split("{}").map(String::from).collect();

    let part_lits: Vec<proc_macro2::Literal> = parts
        .iter()
        .map(|part| proc_macro2::Literal::string(part))
        .collect();

    let crate_path = crate_path();
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
