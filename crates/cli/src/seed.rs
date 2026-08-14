//! Declarative, idempotent database seeding.

use std::borrow::Cow;
use std::sync::Arc;

use ruprizzle::types::chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use ruprizzle::{Executor, Pool, Tx, Value};
use ruprizzle_core::ir::{Field, FieldKind, Model, ScalarType, Schema};
use ruprizzle_dialect::DbDialect;
use serde_json::Value as JsonValue;

/// Applies a declarative JSON seed document in one transaction.
///
/// The top-level object maps model names or physical table names to arrays of
/// row objects. Every row must include the model's primary-key fields. Existing
/// rows are updated on the primary-key conflict, so running the same seed twice
/// is safe.
///
/// # Errors
///
/// Returns an error when the document does not match the schema or a database
/// statement fails.
pub async fn apply(
    schema: &Schema,
    document: &str,
    pool: &Pool,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let root: serde_json::Map<String, JsonValue> = serde_json::from_str(document)?;
    for name in root.keys() {
        if !schema
            .models
            .values()
            .any(|model| model.name.as_str() == name || model.table == *name)
        {
            return Err(
                format!("seed entry `{name}` does not match a schema model or table").into(),
            );
        }
    }
    let dialect = pool.dialect();
    let tx = Tx::begin(pool).await?;
    let mut applied = 0;

    for model in schema.models.values() {
        let Some(rows) = find_rows(&root, model) else {
            continue;
        };
        let rows = rows
            .as_array()
            .ok_or_else(|| format!("seed entry for `{}` must be an array", model.name))?;
        for row in rows {
            let object = row
                .as_object()
                .ok_or_else(|| format!("seed row for `{}` must be an object", model.name))?;
            let (sql, binds) = compile_row(model, object, dialect)?;
            tx.execute_raw(Cow::Owned(sql), binds).await?;
            applied += 1;
        }
    }

    tx.commit().await?;
    Ok(applied)
}

fn find_rows<'a>(
    root: &'a serde_json::Map<String, JsonValue>,
    model: &Model,
) -> Option<&'a JsonValue> {
    root.get(model.name.as_str())
        .or_else(|| root.get(&model.table))
}

fn compile_row(
    model: &Model,
    row: &serde_json::Map<String, JsonValue>,
    dialect: &dyn DbDialect,
) -> Result<(String, Vec<Value>), Box<dyn std::error::Error + Send + Sync>> {
    for key in row.keys() {
        if !model
            .fields
            .values()
            .any(|field| field.name.as_str() == key || field.column == *key)
        {
            return Err(format!("seed column `{key}` does not exist on `{}`", model.name).into());
        }
    }

    let mut fields = Vec::new();
    for field in model.scalar_fields() {
        if let Some(value) = row
            .get(field.name.as_str())
            .or_else(|| row.get(&field.column))
        {
            fields.push((field, json_to_value(value, field)?));
        }
    }
    if fields.is_empty() {
        return Err(format!("seed row for `{}` has no known columns", model.name).into());
    }

    let primary_key = model
        .primary_key
        .fields
        .iter()
        .map(|name| {
            model
                .field(name.as_str())
                .map(|field| field.column.clone())
                .ok_or_else(|| {
                    format!(
                        "primary-key field `{name}` is missing from `{}`",
                        model.name
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for key in &primary_key {
        if !fields.iter().any(|(field, _)| field.column == *key) {
            return Err(format!(
                "seed row for `{}` must include primary key `{key}`",
                model.name
            )
            .into());
        }
    }

    let columns = fields
        .iter()
        .map(|(field, _)| dialect.quote_ident(&field.column))
        .collect::<Vec<_>>();
    let placeholders = (0..fields.len())
        .map(|index| dialect.placeholder(index))
        .collect::<Vec<_>>();
    let update = fields
        .iter()
        .filter(|(field, _)| !primary_key.contains(&field.column))
        .map(|(field, _)| field.column.clone())
        .collect::<Vec<_>>();
    let conflict = primary_key
        .iter()
        .map(|column| dialect.quote_ident(column))
        .collect::<Vec<_>>();
    let clause = if dialect.name() == "mysql" {
        mysql_upsert_clause(dialect, &update, &primary_key)
    } else if update.is_empty() {
        format!("ON CONFLICT ({}) DO NOTHING", conflict.join(", "))
    } else {
        let assignments = update
            .iter()
            .map(|column| {
                let quoted = dialect.quote_ident(column);
                format!("{quoted} = excluded.{quoted}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "ON CONFLICT ({}) DO UPDATE SET {assignments}",
            conflict.join(", ")
        )
    };

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) {}",
        dialect.quote_ident(&model.table),
        columns.join(", "),
        placeholders.join(", "),
        clause
    );
    Ok((sql, fields.into_iter().map(|(_, value)| value).collect()))
}

fn mysql_upsert_clause(
    dialect: &dyn DbDialect,
    update: &[String],
    primary_key: &[String],
) -> String {
    if update.is_empty() {
        let column = dialect.quote_ident(primary_key.first().map_or("id", String::as_str));
        return format!("ON DUPLICATE KEY UPDATE {column} = {column}");
    }
    let assignments = update
        .iter()
        .map(|column| {
            let quoted = dialect.quote_ident(column);
            format!("{quoted} = VALUES({quoted})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("ON DUPLICATE KEY UPDATE {assignments}")
}

fn json_to_value(
    value: &JsonValue,
    field: &Field,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let scalar = match &field.kind {
        FieldKind::Scalar(scalar) => *scalar,
        FieldKind::Enum(_) | FieldKind::Relation(_) | FieldKind::List(_) => ScalarType::String,
    };
    match scalar {
        ScalarType::String | ScalarType::Boolean => match scalar {
            ScalarType::String => Ok(Value::Str(Arc::from(json_string(value)?))),
            ScalarType::Boolean => {
                Ok(Value::Bool(value.as_bool().ok_or_else(|| {
                    format!("seed value for `{}` must be boolean", field.name)
                })?))
            }
            _ => unreachable!(),
        },
        ScalarType::Int => Ok(Value::I32(
            value
                .as_i64()
                .ok_or_else(|| format!("seed value for `{}` must be an integer", field.name))?
                .try_into()?,
        )),
        ScalarType::BigInt => {
            Ok(Value::I64(value.as_i64().ok_or_else(|| {
                format!("seed value for `{}` must be an integer", field.name)
            })?))
        }
        ScalarType::Float => {
            Ok(Value::F64(value.as_f64().ok_or_else(|| {
                format!("seed value for `{}` must be a number", field.name)
            })?))
        }
        ScalarType::Decimal => Ok(Value::Decimal(json_string(value)?.parse()?)),
        ScalarType::DateTime => Ok(Value::DateTime(
            DateTime::parse_from_rfc3339(&json_string(value)?)?.with_timezone(&Utc),
        )),
        ScalarType::Date => Ok(Value::Date(NaiveDate::parse_from_str(
            &json_string(value)?,
            "%Y-%m-%d",
        )?)),
        ScalarType::Time => Ok(Value::Time(NaiveTime::parse_from_str(
            &json_string(value)?,
            "%H:%M:%S%.f",
        )?)),
        ScalarType::Uuid => Ok(Value::Uuid(json_string(value)?.parse()?)),
        ScalarType::Json => Ok(Value::Json(value.clone())),
        ScalarType::Bytes => Ok(Value::Bytes(Arc::from(json_string(value)?.into_bytes()))),
    }
}

fn json_string(value: &JsonValue) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "seed value must be a string".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use ruprizzle_core::ir::{FieldAttrs, FieldKind, Model, PrimaryKey, Provider};
    use ruprizzle_core::names::{FieldName, ModelName};
    use ruprizzle_core::span::Span;

    fn model() -> Model {
        let mut fields = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            Field {
                name: FieldName::new("id"),
                column: "id".to_owned(),
                kind: FieldKind::Scalar(ScalarType::Int),
                optional: false,
                default: None,
                attrs: FieldAttrs {
                    is_id: true,
                    ..FieldAttrs::default()
                },
                docs: None,
                span: Span::EMPTY,
            },
        );
        fields.insert(
            FieldName::new("name"),
            Field {
                name: FieldName::new("name"),
                column: "name".to_owned(),
                kind: FieldKind::Scalar(ScalarType::String),
                optional: false,
                default: None,
                attrs: FieldAttrs::default(),
                docs: None,
                span: Span::EMPTY,
            },
        );
        Model {
            name: ModelName::new("User"),
            table: "users".to_owned(),
            fields,
            primary_key: PrimaryKey {
                fields: vec![FieldName::new("id")],
                name: None,
                span: Span::EMPTY,
            },
            indexes: Vec::new(),
            uniques: Vec::new(),
            docs: None,
            span: Span::EMPTY,
        }
    }

    #[tokio::test]
    async fn apply_is_transactional_and_idempotent() {
        use std::borrow::Cow;

        let source = r#"
            datasource db { provider = "sqlite" url = env("DATABASE_URL") }
            generator client { provider = "rust" }
            model User {
                id Int @id
                name String
            }
        "#;
        let schema = ruprizzle_parser::parse("seed", source).unwrap();
        let mut config = ruprizzle::PoolConfig::default();
        config.max_connections = 1;
        let pool = ruprizzle::connect_with("sqlite::memory:", &config)
            .await
            .unwrap();
        pool.execute_raw(
            Cow::Owned(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_owned(),
            ),
            Vec::new(),
        )
        .await
        .unwrap();

        apply(&schema, r#"{"User":[{"id":1,"name":"Alice"}]}"#, &pool)
            .await
            .unwrap();
        apply(&schema, r#"{"User":[{"id":1,"name":"Bob"}]}"#, &pool)
            .await
            .unwrap();

        let count: i64 = ruprizzle::sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(pool.as_sqlite().unwrap())
            .await
            .unwrap();
        let name: String = ruprizzle::sqlx::query_scalar("SELECT name FROM users WHERE id = 1")
            .fetch_one(pool.as_sqlite().unwrap())
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(name, "Bob");
        pool.close().await;
    }

    #[test]
    fn compile_seed_row_is_idempotent_upsert() {
        let row = serde_json::json!({"id": 1, "name": "Alice"});
        let (sql, binds) = compile_row(
            &model(),
            row.as_object().unwrap(),
            ruprizzle_dialect::dialect_for(Provider::Sqlite),
        )
        .unwrap();
        assert!(sql.contains("ON CONFLICT"));
        assert_eq!(binds, vec![Value::I32(1), Value::Str(Arc::from("Alice"))]);
    }
}
