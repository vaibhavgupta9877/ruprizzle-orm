//! Property tests for the diff engine.
//!
//! Hand-written diff tests only cover transitions someone thought of. These
//! generate the transitions instead. Schemas are built by rendering DSL text
//! and parsing it, which exercises the parser on the same path users take.

use proptest::prelude::*;
use ruprizzle_core::ir::Schema;
use ruprizzle_dialect::dialect_for;
use ruprizzle_migrate::{diff, up_sql};
use ruprizzle_parser::parse;

/// A field that may appear on the generated model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    name: String,
    ty: &'static str,
    optional: bool,
    unique: bool,
}

fn field_strategy() -> impl Strategy<Value = Field> {
    (
        "[a-z][a-z0-9]{2,7}",
        prop::sample::select(vec!["String", "Int", "BigInt", "Boolean", "DateTime"]),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(name, ty, optional, unique)| Field {
            name,
            ty,
            optional,
            unique,
        })
}

/// Renders a schema with the given fields on a single model.
fn render(fields: &[Field]) -> String {
    let mut s = String::from(
        "datasource db {\n  provider = \"postgres\"\n  url = \"postgres://x/y\"\n}\n\n\
         generator client {\n  provider = \"rust\"\n}\n\n\
         model Thing {\n  id Int @id\n",
    );
    for f in fields {
        s.push_str(&format!(
            "  {} {}{}{}\n",
            f.name,
            f.ty,
            if f.optional { "?" } else { "" },
            if f.unique { " @unique" } else { "" }
        ));
    }
    s.push_str("}\n");
    s
}

/// Parses, or returns `None` if the generated text was not valid (duplicate
/// field names are the expected cause, and are not interesting to the property).
fn schema_of(fields: &[Field]) -> Option<Schema> {
    parse("prop", &render(fields)).ok()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Diffing a schema against itself must produce no changes. A false
    /// positive here means `migrate dev` writes empty migrations forever.
    #[test]
    fn diff_with_self_is_empty(fields in prop::collection::vec(field_strategy(), 0..6)) {
        let Some(s) = schema_of(&fields) else { return Ok(()); };
        prop_assert!(diff(&s, &s).is_empty(), "self-diff produced changes");
    }

    /// Any change between two schemas must produce SQL. A silent empty diff
    /// between different schemas is how a column quietly never gets created.
    #[test]
    fn different_schemas_produce_sql(
        a in prop::collection::vec(field_strategy(), 0..5),
        b in prop::collection::vec(field_strategy(), 0..5),
    ) {
        let (Some(sa), Some(sb)) = (schema_of(&a), schema_of(&b)) else { return Ok(()); };
        let changes = diff(&sa, &sb);
        if !changes.is_empty() {
            let dialect = dialect_for(sa.datasource.provider);
            let sql = up_sql(&sa, &sb, dialect.as_ref());
            prop_assert!(
                !sql.trim().is_empty(),
                "diff reported {} changes but produced no SQL",
                changes.len()
            );
        }
    }
}

use ruprizzle_migrate::detect;

/// A schema with no models, used as the starting point for a round trip.
fn empty_schema() -> String {
    "datasource db {\n  provider = \"postgres\"\n  url = \"postgres://x/y\"\n}\n\n\
     generator client {\n  provider = \"rust\"\n}\n"
        .to_owned()
}

/// Builds an empty-to-`from` migration and applies it, then diffs to `to`
/// and applies that. The database must then report no drift against `to`.
async fn round_trip(url: &str, from: &Schema, to: &Schema) -> Result<Vec<String>, String> {
    let pool = ruprizzle::connect(url).await.map_err(|e| e.to_string())?;
    let dialect = dialect_for(from.datasource.provider);

    // Isolate each case: drop anything a previous case left behind. The model
    // is always called "Thing", but the physical table name is pluralised, so
    // we read it out of the schema and quote it through the dialect.
    let table = from
        .model("Thing")
        .ok_or_else(|| "model Thing missing in from schema".to_owned())?
        .table
        .clone();
    let drop_sql = dialect
        .drop_table(&table)
        .into_iter()
        .next()
        .ok_or_else(|| "drop_table produced no statement".to_owned())?
        .sql;
    sqlx::query(&drop_sql)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let empty = parse("empty", &empty_schema()).map_err(|_| "parse empty".to_owned())?;
    for sql in [
        up_sql(&empty, from, dialect.as_ref()),
        up_sql(from, to, dialect.as_ref()),
    ] {
        for stmt in ruprizzle_migrate::runner::split_statements(&sql) {
            sqlx::query(&stmt)
                .execute(&pool)
                .await
                .map_err(|e| format!("{stmt}: {e}"))?;
        }
    }

    detect(&pool, to).await.map_err(|e| e.to_string())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Applying the generated migration must actually reach the target schema.
    #[test]
    fn applied_diff_reaches_the_target_schema(
        a in prop::collection::vec(field_strategy(), 0..4),
        b in prop::collection::vec(field_strategy(), 0..4),
    ) {
        let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") else { return Ok(()); };
        let (Some(sa), Some(sb)) = (schema_of(&a), schema_of(&b)) else { return Ok(()); };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        match rt.block_on(round_trip(&url, &sa, &sb)) {
            Ok(drift) => prop_assert!(
                drift.is_empty(),
                "after applying the diff, drift remains: {drift:?}"
            ),
            Err(e) => prop_assert!(false, "round trip failed: {e}"),
        }
    }
}
