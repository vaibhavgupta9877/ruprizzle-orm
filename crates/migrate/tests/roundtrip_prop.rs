//! Property tests for the diff engine.
//!
//! Hand-written diff tests only cover transitions someone thought of. These
//! generate the transitions instead. Schemas are built by rendering DSL text
//! and parsing it, which exercises the parser on the same path users take.
//!
//! The DB-backed round-trip property in this file is **Postgres-only**. The
//! brief for PR-13 explicitly pinned the property to a live Postgres database,
//! and the schema/render code below is written for `provider = "postgres"`.
//! SQLite coverage for the same property is deferred; see the note in
//! `ProjectPlan/ImplementationPlan/ImplPlan10AppendixDecisions.md`.

use std::borrow::Cow;
use std::sync::OnceLock;
use std::time::Duration;

use proptest::prelude::*;
use ruprizzle::Executor;
use ruprizzle_core::ir::Schema;
use ruprizzle_dialect::dialect_for;
use ruprizzle_migrate::{diff, up_sql};
use ruprizzle_parser::parse;
use ruprizzle_testkit::IsolatedSchema;

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
            let sql = up_sql(&sa, &sb, dialect);
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

/// Pool settings that fail fast when Postgres is not reachable.
fn short_pool_config() -> ruprizzle::PoolConfig {
    let mut config = ruprizzle::PoolConfig::default();
    config.max_connections = 2;
    config.acquire_timeout = Duration::from_secs(5);
    config
}

/// Cached reachability probe for the Postgres test URL.
///
/// A short-timeout probe keeps the suite from hanging for 30 seconds per case
/// when `RUPRIZZLE_TEST_PG_URL` is set but the database is unreachable.
static PG_REACHABLE: OnceLock<Result<(), String>> = OnceLock::new();

/// Probes the configured Postgres URL using a short-timeout pool, then closes
/// it. Returns `Ok(())` only if a real connection and `SELECT 1` succeed.
async fn probe_db(url: &str, config: &ruprizzle::PoolConfig) -> Result<(), String> {
    let pool = ruprizzle::connect_with(url, config)
        .await
        .map_err(|e| e.to_string())?;
    ruprizzle::ping(&pool).await.map_err(|e| e.to_string())?;
    pool.close().await;
    Ok(())
}

/// Builds an empty-to-`from` migration and applies it, then diffs to `to`
/// and applies that. The schema must then report no drift against `to`.
///
/// Runs in a private Postgres schema so the drift check observes only this
/// case's tables.
async fn round_trip(
    url: &str,
    config: &ruprizzle::PoolConfig,
    from: &Schema,
    to: &Schema,
) -> Result<Vec<String>, String> {
    // Each case runs in its own freshly-created schema. This used to run in
    // `public` and "isolate" itself by dropping the single table it knew about,
    // which left the drift assertion below reading a schema that every other
    // DB-backed test in the workspace also writes into: any table they left
    // behind was reported as drift and failed the property. Drift detection
    // scopes itself with `current_schema()`, so a private schema makes the
    // assertion see exactly the tables this case created.
    let schema = IsolatedSchema::create(url)
        .await
        .map_err(|e| e.to_string())?;
    let pool = ruprizzle::connect_with(schema.url(), config)
        .await
        .map_err(|e| e.to_string())?;
    let dialect = dialect_for(from.datasource.provider);

    let empty = parse("empty", &empty_schema()).map_err(|_| "parse empty".to_owned())?;
    for sql in [up_sql(&empty, from, dialect), up_sql(from, to, dialect)] {
        for stmt in ruprizzle_migrate::runner::split_statements(&sql) {
            pool.execute_raw(Cow::Owned(stmt.clone()), Vec::new())
                .await
                .map_err(|e| format!("{stmt}: {e}"))?;
        }
    }

    let drift = detect(&pool, to).await.map_err(|e| e.to_string())?;
    pool.close().await;
    schema.drop_now().await.map_err(|e| e.to_string())?;
    Ok(drift)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Applying the generated migration must actually reach the target schema.
    #[test]
    fn applied_diff_reaches_the_target_schema(
        a in prop::collection::vec(field_strategy(), 0..4),
        b in prop::collection::vec(field_strategy(), 0..4),
    ) {
        let required = std::env::var("RUPRIZZLE_REQUIRE_DB").is_ok();
        let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") else {
            prop_assert!(
                !required,
                "RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not"
            );
            return Ok(());
        };

        let config = short_pool_config();

        // Probe once across all cases. If Postgres is unreachable and not
        // required, skip quickly instead of timing out for every case.
        let probe = PG_REACHABLE.get_or_init(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(probe_db(&url, &config))
        });
        if let Err(e) = probe.as_ref() {
            prop_assert!(
                !required,
                "Postgres is required (RUPRIZZLE_REQUIRE_DB is set) but unreachable: {e}"
            );
            return Ok(());
        }

        let (Some(sa), Some(sb)) = (schema_of(&a), schema_of(&b)) else { return Ok(()); };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        match rt.block_on(round_trip(&url, &config, &sa, &sb)) {
            Ok(drift) => prop_assert!(
                drift.is_empty(),
                "after applying the diff, drift remains: {drift:?}"
            ),
            Err(e) => prop_assert!(false, "round trip failed: {e}"),
        }
    }
}
