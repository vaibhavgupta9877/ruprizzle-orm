//! Rendering for `ruprizzle db pull`.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use ruprizzle_core::ir::{DatasourceUrl, Generator, Provider};
use ruprizzle_migrate::introspect::{Column, DatabaseSchema, Table};

/// Renders an introspected database as a parseable `schema.ruprizzle` file.
#[must_use]
pub fn render_schema(
    database: &DatabaseSchema,
    datasource_url: &DatasourceUrl,
    generator: &Generator,
) -> String {
    let mut out = String::new();
    let provider = provider_name(database.provider);
    let _ = writeln!(out, "datasource db {{");
    let _ = writeln!(out, "  provider = \"{provider}\"");
    let _ = writeln!(out, "  url      = {}", render_url(datasource_url));
    let _ = writeln!(out, "}}\n");

    let _ = writeln!(
        out,
        "generator {} {{",
        identifier(&generator.name, "client")
    );
    let _ = writeln!(out, "  provider   = \"rust\"");
    let _ = writeln!(out, "  output     = \"{}\"", escape(&generator.output));
    let _ = writeln!(
        out,
        "  module_name = \"{}\"",
        escape(&generator.module_name)
    );
    let _ = writeln!(out, "}}\n");

    let model_names = unique_model_names(&database.tables);
    let relations = relation_specs(&database.tables, &model_names);
    for table in &database.tables {
        render_model(&mut out, table, &model_names[&table.name], &relations);
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone)]
struct RelationSpec {
    owner_table: String,
    target_table: String,
    owner_model: String,
    target_model: String,
    name: String,
    owner_field: String,
    inverse_field: String,
    columns: Vec<String>,
    target_columns: Vec<String>,
    optional: bool,
    on_delete: Option<String>,
}

fn relation_specs(tables: &[Table], model_names: &HashMap<String, String>) -> Vec<RelationSpec> {
    let scalar_names = tables
        .iter()
        .map(|table| {
            (
                table.name.clone(),
                table
                    .columns
                    .iter()
                    .map(|column| field_name(&column.name))
                    .collect::<HashSet<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut used = scalar_names.clone();
    let mut specs = Vec::new();

    for table in tables {
        for foreign_key in &table.foreign_keys {
            if !model_names.contains_key(&foreign_key.target_table) {
                continue;
            }
            let target_model = &model_names[&foreign_key.target_table];
            let mut owner_field = lower_first(target_model);
            if !used
                .entry(table.name.clone())
                .or_default()
                .insert(owner_field.clone())
            {
                owner_field = format!("{}_by_{}", owner_field, field_name(&foreign_key.columns[0]));
                used.entry(table.name.clone())
                    .or_default()
                    .insert(owner_field.clone());
            }
            let owner_model = &model_names[&table.name];
            let mut inverse_field = field_name(&table.name);
            if !used
                .entry(foreign_key.target_table.clone())
                .or_default()
                .insert(inverse_field.clone())
            {
                inverse_field.push_str("_relation");
                used.entry(foreign_key.target_table.clone())
                    .or_default()
                    .insert(inverse_field.clone());
            }
            specs.push(RelationSpec {
                owner_table: table.name.clone(),
                target_table: foreign_key.target_table.clone(),
                owner_model: owner_model.clone(),
                target_model: target_model.clone(),
                name: format!("{}_{}", table.name, foreign_key.name),
                owner_field,
                inverse_field,
                columns: foreign_key.columns.clone(),
                target_columns: foreign_key.target_columns.clone(),
                optional: foreign_key.columns.iter().any(|column| {
                    table
                        .columns
                        .iter()
                        .find(|candidate| candidate.name == *column)
                        .is_some_and(|candidate| candidate.nullable)
                }),
                on_delete: foreign_key.on_delete.clone(),
            });
        }
    }
    specs
}

fn render_model(out: &mut String, table: &Table, model_name: &str, relations: &[RelationSpec]) {
    let _ = writeln!(out, "model {model_name} {{");
    for column in &table.columns {
        let field_name = field_name(&column.name);
        let mut attrs = Vec::new();
        attrs.push(format!("@map(\"{}\")", escape(&column.name)));
        if column.primary_key && table.primary_key.len() == 1 {
            attrs.push("@id".to_owned());
        }
        if let Some(default) = render_default(column) {
            attrs.push(format!("@default({default})"));
        }
        let optional = if column.nullable { "?" } else { "" };
        let _ = writeln!(
            out,
            "  {field_name} {}{optional} {}",
            scalar_type(column),
            attrs.join(" ")
        );
    }
    for relation in relations
        .iter()
        .filter(|relation| relation.owner_table == table.name)
    {
        let fields = relation
            .columns
            .iter()
            .map(|column| field_name(column))
            .collect::<Vec<_>>()
            .join(", ");
        let references = relation
            .target_columns
            .iter()
            .map(|column| field_name(column))
            .collect::<Vec<_>>()
            .join(", ");
        let action = relation
            .on_delete
            .as_deref()
            .and_then(relation_action)
            .map_or_else(String::new, |action| format!(", onDelete: {action}"));
        let optional = if relation.optional { "?" } else { "" };
        let _ = writeln!(
            out,
            "  {} {}{} @relation(\"{}\", fields: [{}], references: [{}]{action})",
            relation.owner_field,
            relation.target_model,
            optional,
            escape(&relation.name),
            fields,
            references
        );
    }
    for relation in relations
        .iter()
        .filter(|relation| relation.target_table == table.name)
    {
        let _ = writeln!(
            out,
            "  {} {}[] @relation(\"{}\")",
            relation.inverse_field,
            relation.owner_model,
            escape(&relation.name)
        );
    }

    if table.name != model_name {
        let _ = writeln!(out, "  @@map(\"{}\")", escape(&table.name));
    }
    if table.primary_key.len() > 1 {
        let fields = table
            .primary_key
            .iter()
            .map(|column| field_name(column))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "  @@id([{fields}])");
    }
    for index in &table.indexes {
        let fields = index
            .columns
            .iter()
            .map(|column| field_name(column))
            .collect::<Vec<_>>()
            .join(", ");
        if fields.is_empty() {
            continue;
        }
        let directive = if index.unique { "@@unique" } else { "@@index" };
        let _ = writeln!(
            out,
            "  {directive}([{fields}], map: \"{}\")",
            escape(&index.name)
        );
    }
    out.push('}');
}

fn render_url(url: &DatasourceUrl) -> String {
    match url {
        DatasourceUrl::Env(name) => format!("env(\"{}\")", escape(name)),
        DatasourceUrl::Literal(value) => format!("\"{}\"", escape(value)),
    }
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Postgres => "postgres",
        Provider::Sqlite => "sqlite",
        Provider::Mysql => "mysql",
    }
}

fn scalar_type(column: &Column) -> &'static str {
    let ty = column.db_type.to_ascii_lowercase();
    if ty.contains("json") {
        "Json"
    } else if ty.contains("bool") || ty == "tinyint(1)" {
        "Boolean"
    } else if ty.contains("bigint") || ty.contains("int8") {
        "BigInt"
    } else if ty.contains("int") || ty.contains("serial") {
        "Int"
    } else if ty.contains("decimal") || ty.contains("numeric") || ty.contains("money") {
        "Decimal"
    } else if ty.contains("double") || ty.contains("float") || ty == "real" {
        "Float"
    } else if ty == "date" {
        "Date"
    } else if ty.starts_with("time") {
        "Time"
    } else if ty.contains("timestamp") || ty.contains("datetime") {
        "DateTime"
    } else if ty.contains("uuid") || ty == "char(36)" {
        "Uuid"
    } else if ty.contains("blob")
        || ty.contains("binary")
        || ty.contains("bytea")
        || ty.contains("varbinary")
    {
        "Bytes"
    } else {
        "String"
    }
}

fn render_default(column: &Column) -> Option<String> {
    if column.auto_increment {
        return Some("autoincrement()".to_owned());
    }
    let value = column.default.as_deref()?.trim();
    if value.is_empty() {
        return None;
    }

    let lower = value.to_ascii_lowercase();
    if lower.starts_with("nextval(") {
        return Some("autoincrement()".to_owned());
    }
    if lower.starts_with("current_timestamp")
        || lower == "now()"
        || lower.starts_with("datetime('now')")
    {
        return Some("now()".to_owned());
    }
    if matches!(lower.as_str(), "true" | "false") {
        return Some(lower);
    }
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        return Some(value.to_owned());
    }

    let unquoted = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(value)
        .trim();
    if unquoted.len() >= 2 && unquoted.starts_with('\'') && unquoted.ends_with('\'') {
        let content = unquoted[1..unquoted.len() - 1].replace("''", "'");
        return Some(format!("\"{}\"", escape(&content)));
    }
    Some(format!("dbgenerated(\"{}\")", escape(value)))
}

fn lower_first(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

fn relation_action(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "cascade" => Some("Cascade"),
        "restrict" => Some("Restrict"),
        "set null" => Some("SetNull"),
        "set default" => Some("SetDefault"),
        "no action" => Some("NoAction"),
        _ => None,
    }
}

fn field_name(column: &str) -> String {
    let mut name = String::new();
    for part in column.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if !part.is_empty() {
            if !name.is_empty() {
                name.push('_');
            }
            name.push_str(part);
        }
    }
    if name.is_empty() {
        name.push_str("field");
    }
    if name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        name.insert_str(0, "field_");
    }
    name
}

fn unique_model_names(tables: &[Table]) -> HashMap<String, String> {
    let mut used = HashSet::new();
    let mut names = HashMap::new();
    for table in tables {
        let base = identifier(&pascal_case(&table.name), "Model");
        let mut name = base.clone();
        let mut suffix = 2;
        while !used.insert(name.clone()) {
            name = format!("{base}{suffix}");
            suffix += 1;
        }
        names.insert(table.name.clone(), name);
    }
    names
}

fn pascal_case(value: &str) -> String {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect()
}

fn identifier(value: &str, fallback: &str) -> String {
    let value = if value.is_empty() { fallback } else { value };
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if (ch.is_ascii_alphanumeric() || ch == '_') && !(index == 0 && ch.is_ascii_digit()) {
            output.push(ch);
        } else if index == 0 {
            output.push_str("Model");
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        fallback.to_owned()
    } else {
        output
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruprizzle_core::ir::{DatasourceUrl, Generator};
    use ruprizzle_migrate::introspect::Index;

    #[tokio::test]
    async fn pulls_sqlite_table_metadata() {
        use ruprizzle::Executor;
        use std::borrow::Cow;

        let path =
            std::env::temp_dir().join(format!("ruprizzle-pull-{}.sqlite", std::process::id()));
        let _ = std::fs::File::create(&path).unwrap();
        let file = path.to_string_lossy().replace('\\', "/");
        let url = format!("sqlite:///{file}?mode=rwc");
        let pool = ruprizzle::connect(&url).await.unwrap();
        pool.execute_raw(
            Cow::Owned(
                "CREATE TABLE authors (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)"
                    .to_owned(),
            ),
            Vec::new(),
        )
        .await
        .unwrap();
        pool.execute_raw(
            Cow::Owned(
                "CREATE TABLE blog_posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, slug TEXT NOT NULL UNIQUE, published_at TEXT, author_id INTEGER REFERENCES authors(id) ON DELETE CASCADE)".to_owned(),
            ),
            Vec::new(),
        )
        .await
        .unwrap();
        pool.execute_raw(
            Cow::Owned(
                "CREATE INDEX blog_posts_published_idx ON blog_posts(published_at)".to_owned(),
            ),
            Vec::new(),
        )
        .await
        .unwrap();

        let database = ruprizzle_migrate::introspect::pull(&pool).await.unwrap();
        let table = database
            .tables
            .iter()
            .find(|table| table.name == "blog_posts")
            .unwrap();
        assert_eq!(table.primary_key, vec!["id"]);
        assert!(table.columns.iter().any(|column| column.auto_increment));
        assert!(
            table
                .indexes
                .iter()
                .any(|index| index.name == "blog_posts_published_idx")
        );
        assert!(
            table
                .indexes
                .iter()
                .any(|index| index.unique && index.columns == ["slug"])
        );
        let foreign_key = table.foreign_keys.first().unwrap();
        assert_eq!(foreign_key.target_table, "authors");
        assert_eq!(foreign_key.columns, ["author_id"]);
        assert_eq!(foreign_key.on_delete.as_deref(), Some("CASCADE"));
        pool.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn renders_nontrivial_schema_that_parser_accepts() {
        let database = DatabaseSchema {
            provider: Provider::Sqlite,
            tables: vec![
                Table {
                    name: "authors".to_owned(),
                    columns: vec![Column {
                        name: "id".to_owned(),
                        db_type: "INTEGER".to_owned(),
                        nullable: false,
                        default: None,
                        auto_increment: true,
                        primary_key: true,
                    }],
                    primary_key: vec!["id".to_owned()],
                    indexes: Vec::new(),
                    foreign_keys: Vec::new(),
                },
                Table {
                    name: "blog_posts".to_owned(),
                    columns: vec![
                        Column {
                            name: "id".to_owned(),
                            db_type: "INTEGER".to_owned(),
                            nullable: false,
                            default: None,
                            auto_increment: true,
                            primary_key: true,
                        },
                        Column {
                            name: "author_id".to_owned(),
                            db_type: "INTEGER".to_owned(),
                            nullable: false,
                            default: None,
                            auto_increment: false,
                            primary_key: false,
                        },
                        Column {
                            name: "published_at".to_owned(),
                            db_type: "TEXT".to_owned(),
                            nullable: true,
                            default: None,
                            auto_increment: false,
                            primary_key: false,
                        },
                    ],
                    primary_key: vec!["id".to_owned()],
                    indexes: vec![Index {
                        name: "blog_posts_published_idx".to_owned(),
                        unique: false,
                        columns: vec!["published_at".to_owned()],
                    }],
                    foreign_keys: vec![ruprizzle_migrate::introspect::ForeignKey {
                        name: "blog_posts_author_id_fkey".to_owned(),
                        columns: vec!["author_id".to_owned()],
                        target_table: "authors".to_owned(),
                        target_columns: vec!["id".to_owned()],
                        on_delete: Some("CASCADE".to_owned()),
                    }],
                },
            ],
        };
        let generator = Generator::default();
        let source = render_schema(
            &database,
            &DatasourceUrl::Env("DATABASE_URL".to_owned()),
            &generator,
        );
        let parsed = ruprizzle_parser::parse("pulled", &source).expect("rendered schema parses");
        assert_eq!(parsed.models.len(), 2);
        assert_eq!(parsed.model("BlogPosts").unwrap().table, "blog_posts");
        assert_eq!(parsed.relations.len(), 1);
        assert!(source.contains("@@index([published_at], map: \"blog_posts_published_idx\")"));
        assert!(source.contains("onDelete: Cascade"));
    }
}
