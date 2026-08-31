#![cfg(feature = "studio")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ruprizzle_cli::studio::handlers::AppState;
use ruprizzle_cli::studio::{
    StudioConfig, create_router, is_loopback_host, looks_like_production_url,
};
use std::sync::Arc;
use tower::ServiceExt;

fn sample_schema() -> ruprizzle_core::ir::Schema {
    let schema_str = r#"
datasource db {
  provider = "sqlite"
  url = "file:dev.db"
}

generator client {
  output = "./src/db"
}

model User {
  id    Int    @id @default(autoincrement())
  email String @unique
  name  String
  posts Post[]
}

model Post {
  id       Int     @id @default(autoincrement())
  title    String
  authorId Int
  author   User    @relation(fields: [authorId], references: [id])
}
"#;

    ruprizzle_parser::parse("schema.ruprizzle", schema_str).expect("schema must parse")
}

#[tokio::test]
async fn the_production_name_check_matches_what_it_claims_to() {
    assert!(looks_like_production_url(
        "postgres://prod-user:pass@prod.db.com/db"
    ));
    assert!(looks_like_production_url(
        "postgresql://user:pass@ep-cool-lake.us-east-2.aws.neon.tech/neondb"
    ));
    assert!(looks_like_production_url("libsql://my-db.turso.io"));
    assert!(!looks_like_production_url("sqlite://dev.db"));
    assert!(!looks_like_production_url(
        "postgres://postgres:postgres@localhost:5432/mydb"
    ));

    // Pinning what it does NOT catch, so nobody mistakes it for a safeguard:
    // a production RDS endpoint and a bare IP both sail through.
    assert!(!looks_like_production_url(
        "postgres://u:p@app-db.cluster-abc.eu-west-1.rds.amazonaws.com/app"
    ));
    assert!(!looks_like_production_url(
        "postgres://u:p@10.0.4.17:5432/app"
    ));
    // ...and it false-positives on a development database.
    assert!(looks_like_production_url(
        "postgres://u:p@localhost/product_catalog_dev"
    ));
}

#[test]
fn loopback_detection_covers_the_forms_a_user_actually_types() {
    for host in [
        "127.0.0.1",
        "localhost",
        "LOCALHOST",
        "::1",
        "[::1]",
        "127.0.0.2",
    ] {
        assert!(is_loopback_host(host), "{host} is loopback");
    }
    for host in ["0.0.0.0", "192.168.1.10", "::", "example.com"] {
        assert!(!is_loopback_host(host), "{host} is not loopback");
    }
}

#[tokio::test]
async fn test_studio_dashboard_and_erd_endpoints() {
    let schema = sample_schema();
    let config = StudioConfig {
        allow_writes: false,
        ..Default::default()
    };
    let state = Arc::new(AppState::new(schema, config, None));
    let app = create_router(state);

    // Test Dashboard (GET /studio)
    let req = Request::builder()
        .uri("/studio")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("Ruprizzle Studio"));
    assert!(body_str.contains("User"));
    assert!(body_str.contains("Post"));

    // Test ERD Data (GET /studio/erd/data)
    let req = Request::builder()
        .uri("/studio/erd/data")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains(r#""name":"User""#));
    assert!(body_str.contains(r#""name":"Post""#));
    assert!(body_str.contains(r#""from":"Post","to":"User""#));
}

#[tokio::test]
async fn table_browser_says_so_instead_of_inventing_rows_without_a_connection() {
    let schema = sample_schema();
    let config = StudioConfig {
        allow_writes: false,
        ..Default::default()
    };
    let state = Arc::new(AppState::new(schema, config, None));
    let app = create_router(state);

    for uri in ["/studio/models/User", "/studio/models/User/table"] {
        let body = get_body(app.clone(), uri).await;
        assert!(body.contains("No rows were read"), "{uri}: {body}");
        assert!(
            !body.contains("Sample "),
            "{uri} still renders fixtures: {body}"
        );
    }
}

#[tokio::test]
async fn test_studio_write_guardrails() {
    let schema = sample_schema();

    // Read-only state
    let read_only_state = Arc::new(AppState::new(
        schema.clone(),
        StudioConfig {
            allow_writes: false,
            ..Default::default()
        },
        None,
    ));
    let read_only_app = create_router(read_only_state);

    let req = Request::builder()
        .method("POST")
        .uri("/studio/models/User/rows")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("name=Alice&email=alice@example.com"))
        .unwrap();
    let res = read_only_app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Writes enabled but no connection: the write must be refused, not faked.
    let disconnected_state = Arc::new(AppState::new(
        schema,
        StudioConfig {
            allow_writes: true,
            ..Default::default()
        },
        None,
    ));
    let disconnected_app = create_router(disconnected_state);

    let req = Request::builder()
        .method("POST")
        .uri("/studio/models/User/rows")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("name=Alice&email=alice@example.com"))
        .unwrap();
    let res = disconnected_app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_studio_static_assets() {
    let schema = sample_schema();
    let state = Arc::new(AppState::new(schema, StudioConfig::default(), None));
    let app = create_router(state);

    // CSS asset
    let req = Request::builder()
        .uri("/studio/assets/css/studio.css")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // JS asset
    let req = Request::builder()
        .uri("/studio/assets/js/htmx.min.js")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// Creates a SQLite database whose shape deliberately disagrees with `sample_schema`:
/// `users` carries an extra populated `legacy_notes` column, and `posts` does not exist.
async fn drifted_pool(dir: &std::path::Path) -> ruprizzle::Pool {
    let path = dir.join("studio.db").to_string_lossy().replace('\\', "/");
    let pool = ruprizzle::connect(&format!("sqlite://{path}?mode=rwc"))
        .await
        .expect("sqlite must connect");

    for sql in [
        "CREATE TABLE \"users\" (id INTEGER PRIMARY KEY, email TEXT NOT NULL, name TEXT NOT NULL, legacy_notes TEXT)",
        "INSERT INTO \"users\" (id, email, name, legacy_notes) VALUES (1, 'a@b.c', 'Alice', 'keep me')",
    ] {
        ruprizzle::Executor::execute_raw(&pool, std::borrow::Cow::Borrowed(sql), Vec::new())
            .await
            .expect("setup statement must run");
    }

    pool
}

async fn get_body(app: axum::Router, uri: &str) -> String {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&body).into_owned()
}

#[tokio::test]
async fn diff_reports_real_drift_against_a_live_database() {
    let dir = tempfile::tempdir().unwrap();
    let pool = drifted_pool(dir.path()).await;

    let state = Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        Some(pool),
    ));
    let body = get_body(create_router(state), "/studio/diff").await;

    // The dropped column holds a value, so it must be called destructive by name,
    // with the real row count behind it.
    assert!(body.contains("legacy_notes"), "{body}");
    assert!(body.contains("DESTRUCTIVE"), "{body}");
    assert!(body.contains("destroys the 1 row(s)"), "{body}");

    // The missing table is a genuine create, and genuinely safe.
    assert!(body.contains("Create table `posts`"), "{body}");

    // The old unconditional card must be gone.
    assert!(!body.contains("structure verified"), "{body}");
}

#[tokio::test]
async fn diff_refuses_to_report_safety_without_a_connection() {
    let state = Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        None,
    ));
    let body = get_body(create_router(state), "/studio/diff").await;

    assert!(body.contains("No comparison was made"), "{body}");
    assert!(!body.contains("SAFE"), "{body}");
    assert!(!body.contains("Zero drift detected"), "{body}");
}

/// A database matching `sample_schema`, seeded with two users and one post.
async fn seeded_pool(dir: &std::path::Path) -> ruprizzle::Pool {
    let path = dir.join("seeded.db").to_string_lossy().replace('\\', "/");
    let pool = ruprizzle::connect(&format!("sqlite://{path}?mode=rwc"))
        .await
        .expect("sqlite must connect");

    for sql in [
        "CREATE TABLE \"users\" (id INTEGER PRIMARY KEY, email TEXT NOT NULL, name TEXT NOT NULL)",
        "CREATE TABLE \"posts\" (id INTEGER PRIMARY KEY, title TEXT NOT NULL, authorId INTEGER NOT NULL)",
        "INSERT INTO \"users\" (id, email, name) VALUES (1, 'alice@example.com', 'Alice')",
        "INSERT INTO \"users\" (id, email, name) VALUES (2, 'bob@example.com', 'Bob')",
        "INSERT INTO \"posts\" (id, title, authorId) VALUES (10, 'Hello', 1)",
    ] {
        ruprizzle::Executor::execute_raw(&pool, std::borrow::Cow::Borrowed(sql), Vec::new())
            .await
            .expect("setup statement must run");
    }

    pool
}

fn writable_app(pool: ruprizzle::Pool) -> axum::Router {
    create_router(Arc::new(AppState::new(
        sample_schema(),
        StudioConfig {
            allow_writes: true,
            ..Default::default()
        },
        Some(pool),
    )))
}

async fn scalar(pool: &ruprizzle::Pool, sql: &'static str) -> String {
    let batch =
        ruprizzle::Executor::fetch_all_raw(pool, std::borrow::Cow::Borrowed(sql), Vec::new())
            .await
            .expect("query must run");
    match batch {
        ruprizzle::RowBatch::Sqlite(rows) => {
            use ruprizzle::sqlx::Row as _;
            rows.first()
                .map(|r| r.try_get::<String, _>(0).expect("column 0 must be text"))
                .unwrap_or_default()
        }
        other => panic!("unexpected batch: {other:?}"),
    }
}

#[tokio::test]
async fn table_browser_renders_the_rows_that_are_in_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    let body = get_body(app, "/studio/models/User/table").await;
    assert!(body.contains("alice@example.com"), "{body}");
    assert!(body.contains("bob@example.com"), "{body}");
    // The fixture generator that produced these is gone.
    assert!(!body.contains("Sample email"), "{body}");
}

#[tokio::test]
async fn table_browser_search_filters_against_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    let body = get_body(app, "/studio/models/User/table?search=bob").await;
    assert!(body.contains("bob@example.com"), "{body}");
    assert!(!body.contains("alice@example.com"), "{body}");
}

#[tokio::test]
async fn patching_a_cell_writes_it_and_shows_what_was_stored() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool.clone());

    let req = Request::builder()
        .method("PATCH")
        .uri("/studio/models/User/rows/1/cell?column=name")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("value=Alicia"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("Alicia"));

    assert_eq!(
        scalar(&pool, "SELECT name FROM \"users\" WHERE id = 1").await,
        "Alicia",
        "the UPDATE must actually reach the database"
    );
}

#[tokio::test]
async fn deleting_a_row_removes_it_and_a_missing_row_is_not_reported_as_success() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool.clone());

    let req = Request::builder()
        .method("DELETE")
        .uri("/studio/models/User/rows/2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        scalar(&pool, "SELECT CAST(COUNT(*) AS TEXT) FROM \"users\"").await,
        "1"
    );

    let req = Request::builder()
        .method("DELETE")
        .uri("/studio/models/User/rows/999")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::NOT_FOUND,
        "deleting nothing must not report OK"
    );
}

#[tokio::test]
async fn inserting_a_row_writes_it_and_reads_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool.clone());

    let req = Request::builder()
        .method("POST")
        .uri("/studio/models/User/rows")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("name=Carol&email=carol@example.com"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("carol@example.com"));

    assert_eq!(
        scalar(
            &pool,
            "SELECT CAST(COUNT(*) AS TEXT) FROM \"users\" WHERE email = 'carol@example.com'"
        )
        .await,
        "1"
    );
}

#[tokio::test]
async fn the_sandbox_runs_the_statement_it_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    let req = Request::builder()
        .method("POST")
        .uri("/studio/sandbox/execute")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("sql=SELECT+email+FROM+%22users%22+ORDER+BY+id"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&body);

    assert!(body.contains("alice@example.com"), "{body}");
    assert!(body.contains("bob@example.com"), "{body}");
    assert!(body.contains("2 row(s)"), "{body}");
    // The old fixed result table is gone.
    assert!(!body.contains("Query Executed Successfully"), "{body}");
}

#[tokio::test]
async fn the_sandbox_reports_a_failing_statement_as_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    let req = Request::builder()
        .method("POST")
        .uri("/studio/sandbox/execute")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("sql=SELECT+*+FROM+no_such_table"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&body);

    assert!(body.contains("Not executed"), "{body}");
    assert!(body.contains("no_such_table"), "{body}");
}

#[tokio::test]
async fn the_sandbox_does_not_execute_mutations_in_read_only_mode() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = create_router(Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        Some(pool.clone()),
    )));

    let req = Request::builder()
        .method("POST")
        .uri("/studio/sandbox/execute")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("sql=DELETE+FROM+%22users%22"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("Nothing was executed"));

    assert_eq!(
        scalar(&pool, "SELECT CAST(COUNT(*) AS TEXT) FROM \"users\"").await,
        "2",
        "the refused DELETE must not have run"
    );
}

#[tokio::test]
async fn explain_returns_the_database_plan_for_the_submitted_query() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    let req = Request::builder()
        .method("POST")
        .uri("/studio/explain")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("sql=SELECT+*+FROM+%22users%22"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&body);

    // SQLite reports a full scan of `users` for this query.
    assert!(body.contains("users"), "{body}");
    // The hardcoded Postgres plan is gone.
    assert!(!body.contains("0.42..12.80"), "{body}");
    assert!(!body.contains("users_pkey"), "{body}");
}

#[tokio::test]
async fn the_relation_drawer_reads_the_record_it_points_at() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    let body = get_body(app.clone(), "/studio/relations/User/1").await;
    assert!(body.contains("alice@example.com"), "{body}");
    assert!(!body.contains("Linked email"), "{body}");

    let body = get_body(app, "/studio/relations/User/999").await;
    assert!(
        body.contains("points at a record that is not there"),
        "{body}"
    );
}
