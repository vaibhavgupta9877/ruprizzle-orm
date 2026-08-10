//! The naming convention: how a declaration becomes a physical name.
//!
//! Applied during lowering, once, so no downstream crate ever re-derives a table
//! or column name. `@map` / `@@map` override everything here.
//!
//! The pluraliser is deliberately dumb. English is not tractable, and a clever
//! implementation would be wrong in unpredictable places; a documented, boring one
//! is wrong in *predictable* places, and `@@map("...")` is the escape hatch.

/// Nouns whose plural is not formed by any rule worth encoding.
const IRREGULAR: &[(&str, &str)] = &[
    ("person", "people"),
    ("child", "children"),
    ("man", "men"),
    ("woman", "women"),
    ("tooth", "teeth"),
    ("foot", "feet"),
    ("mouse", "mice"),
    ("goose", "geese"),
    ("datum", "data"),
    ("medium", "media"),
    ("analysis", "analyses"),
    ("index", "indexes"),
];

/// Nouns that are already their own plural.
const UNCOUNTABLE: &[&str] = &[
    "equipment",
    "information",
    "series",
    "species",
    "news",
    "data",
    "metadata",
    "staff",
    "sheep",
    "fish",
    "money",
    "audio",
    "settings",
];

/// Converts `PascalCase` or `camelCase` to `snake_case`.
///
/// Runs of capitals are treated as one word, so `HTTPHeader` becomes
/// `http_header` rather than `h_t_t_p_header`, and a capital after a digit starts
/// a new word (`v2Endpoint` → `v2_endpoint`).
#[must_use]
pub fn snake_case(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len() + 4);

    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c == '-' || c == ' ' {
            if !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
            continue;
        }
        if c.is_ascii_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_is_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            // A boundary is either lower→upper (`createdAt`) or the last capital
            // of a run that starts a new word (`HTTPHeader`).
            let boundary = prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next_is_lower);
            if boundary && !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Pluralises a lowercase English noun.
#[must_use]
pub fn pluralize(word: &str) -> String {
    if word.is_empty() {
        return String::new();
    }
    let lower = word.to_lowercase();

    if UNCOUNTABLE.contains(&lower.as_str()) {
        return lower;
    }
    if let Some((_, plural)) = IRREGULAR.iter().find(|(s, _)| *s == lower) {
        return (*plural).to_owned();
    }
    // Only the final word of a compound is inflected: `blog_post` → `blog_posts`.
    if let Some((head, tail)) = lower.rsplit_once('_') {
        return format!("{head}_{}", pluralize(tail));
    }

    let sibilant = ["s", "x", "z", "ch", "sh"]
        .iter()
        .any(|suffix| lower.ends_with(suffix));
    let consonant_y = lower.ends_with('y')
        && lower
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|p| !"aeiou".contains(p));

    if sibilant {
        format!("{lower}es")
    } else if consonant_y {
        format!("{}ies", &lower[..lower.len() - 1])
    } else {
        format!("{lower}s")
    }
}

/// The table name for a model with no `@@map`.
#[must_use]
pub fn table_name(model: &str) -> String {
    pluralize(&snake_case(model))
}

/// The column name for a field with no `@map`.
#[must_use]
pub fn column_name(field: &str) -> String {
    snake_case(field)
}

/// The database type name for an enum with no `@@map`.
#[must_use]
pub fn enum_type_name(name: &str) -> String {
    snake_case(name)
}

/// The derived name of an index over `columns` on `table`.
#[must_use]
pub fn index_name(table: &str, columns: &[String]) -> String {
    format!("{table}_{}_idx", columns.join("_"))
}

/// The derived name of a unique constraint over `columns` on `table`.
#[must_use]
pub fn unique_name(table: &str, columns: &[String]) -> String {
    format!("{table}_{}_key", columns.join("_"))
}

/// The derived name of a foreign key constraint over `columns` on `table`.
#[must_use]
pub fn foreign_key_name(table: &str, columns: &[String]) -> String {
    format!("{table}_{}_fkey", columns.join("_"))
}

/// Rust keywords that cannot appear bare in generated code (V17).
///
/// Includes the reserved-but-unused set: a field named `become` compiles today
/// but would break the day the keyword is activated, and the `r#` escape costs
/// nothing.
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", "box", "become", "do", "final", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Whether a field name needs `r#` escaping in generated Rust.
#[must_use]
pub fn is_rust_keyword(name: &str) -> bool {
    RUST_KEYWORDS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_handles_the_shapes_we_actually_see() {
        assert_eq!(snake_case("User"), "user");
        assert_eq!(snake_case("createdAt"), "created_at");
        assert_eq!(snake_case("BlogPost"), "blog_post");
        assert_eq!(snake_case("already_snake"), "already_snake");
        assert_eq!(snake_case("HTTPHeader"), "http_header");
        assert_eq!(snake_case("APIKey"), "api_key");
        assert_eq!(snake_case("v2Endpoint"), "v2_endpoint");
        assert_eq!(snake_case("id"), "id");
    }

    #[test]
    fn pluralize_follows_the_documented_rules() {
        assert_eq!(pluralize("user"), "users");
        assert_eq!(pluralize("post"), "posts");
        assert_eq!(pluralize("category"), "categories");
        assert_eq!(pluralize("address"), "addresses");
        assert_eq!(pluralize("box"), "boxes");
        assert_eq!(pluralize("dish"), "dishes");
        assert_eq!(pluralize("day"), "days");
        assert_eq!(pluralize("person"), "people");
        assert_eq!(pluralize("series"), "series");
        assert_eq!(pluralize("blog_post"), "blog_posts");
    }

    #[test]
    fn table_and_column_names() {
        assert_eq!(table_name("User"), "users");
        assert_eq!(table_name("BlogPost"), "blog_posts");
        assert_eq!(table_name("Category"), "categories");
        assert_eq!(column_name("createdAt"), "created_at");
        assert_eq!(enum_type_name("Role"), "role");
    }

    #[test]
    fn keyword_detection_covers_reserved_words() {
        assert!(is_rust_keyword("type"));
        assert!(is_rust_keyword("become"));
        assert!(!is_rust_keyword("email"));
    }
}
