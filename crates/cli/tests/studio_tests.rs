#![cfg(feature = "studio")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ruprizzle_cli::studio::guard::{TOKEN_HEADER, host_is_allowed};
use ruprizzle_cli::studio::handlers::AppState;
use ruprizzle_cli::studio::{StudioConfig, is_loopback_host, looks_like_production_url};
use std::sync::Arc;
use tower::ServiceExt;

/// What a browser sends for a page served from the default bind.
const HOST: &str = "127.0.0.1:5555";
const ORIGIN: &str = "http://127.0.0.1:5555";
const TOKEN: &str = "test-session-token";

/// Builds Studio's router with a known session token, so tests can send it.
fn create_router(state: Arc<AppState>) -> axum::Router {
    let mut state = (*state).clone();
    state.session_token = TOKEN.to_string();
    ruprizzle_cli::studio::create_router(Arc::new(state))
}

/// A request as the Studio page itself would send it: right `Host`, and for a
/// mutation (after `.method(..)`) the page's `Origin` and session token.
fn studio_request() -> axum::http::request::Builder {
    Request::builder().header("host", HOST)
}

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
    let req = studio_request().uri("/studio").body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("Ruprizzle Studio"));
    assert!(body_str.contains("User"));
    assert!(body_str.contains("Post"));

    // Test ERD Data (GET /studio/erd/data)
    let req = studio_request()
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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
    let req = studio_request()
        .uri("/studio/assets/css/studio.css")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // JS asset
    let req = studio_request()
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
    let req = studio_request().uri(uri).body(Body::empty()).unwrap();
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

    let req = studio_request()
        .method("PATCH")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("DELETE")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("DELETE")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
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

// ---- K1: cross-origin and DNS-rebinding requests are refused ---------------------

async fn user_count(pool: &ruprizzle::Pool) -> String {
    scalar(pool, "SELECT CAST(count(*) AS TEXT) FROM \"users\"").await
}

fn sandbox_delete() -> axum::http::request::Builder {
    studio_request()
        .method("POST")
        .uri("/studio/sandbox/execute")
        .header("content-type", "application/x-www-form-urlencoded")
}

#[tokio::test]
async fn a_cross_origin_form_post_cannot_drive_studio() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool.clone());

    // What a hostile page's auto-submitting <form> sends: its own Origin, no token.
    let req = sandbox_delete()
        .header("origin", "http://evil.example")
        .body(Body::from("sql=DELETE+FROM+%22users%22"))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // A local app on another port is a different origin too.
    let req = sandbox_delete()
        .header("origin", "http://127.0.0.1:3000")
        .header(TOKEN_HEADER, TOKEN)
        .body(Body::from("sql=DELETE+FROM+%22users%22"))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    // No Origin at all is refused as well.
    let req = sandbox_delete()
        .header(TOKEN_HEADER, TOKEN)
        .body(Body::from("sql=DELETE+FROM+%22users%22"))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    // Right Origin, wrong or missing token.
    for token in [Some("guess"), None] {
        let mut req = sandbox_delete().header("origin", ORIGIN);
        if let Some(token) = token {
            req = req.header(TOKEN_HEADER, token);
        }
        let req = req.body(Body::from("sql=DELETE+FROM+%22users%22")).unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }

    // Row deletion through the table route is guarded the same way.
    let req = studio_request()
        .method("DELETE")
        .uri("/studio/models/User/rows/1")
        .header("origin", "http://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    assert_eq!(user_count(&pool).await, "2", "no refused request may write");
}

#[tokio::test]
async fn a_rebound_hostname_cannot_read_studio() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = writable_app(pool);

    // A DNS-rebinding page reaches 127.0.0.1 but still sends its own name.
    let req = Request::builder()
        .uri("/studio/models/User/table")
        .header("host", "attacker.example:5555")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(!String::from_utf8_lossy(&body).contains("alice@example.com"));

    // No Host header at all is refused.
    let req = Request::builder()
        .uri("/studio")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    // `localhost` with a port is the normal case and is served.
    let req = Request::builder()
        .uri("/studio")
        .header("host", "localhost:5555")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn the_page_shell_hands_htmx_the_session_token() {
    let app = create_router(Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        None,
    )));
    let body = get_body(app, "/studio").await;
    assert!(
        body.contains(&format!(r#"hx-headers='{{"x-studio-token": "{TOKEN}"}}'"#)),
        "{body}"
    );
}

#[test]
fn host_check_accepts_studio_names_and_nothing_else() {
    let loopback = StudioConfig::default();
    for host in [
        "127.0.0.1:5555",
        "localhost:5555",
        "LOCALHOST",
        "[::1]:5555",
    ] {
        assert!(host_is_allowed(Some(host), &loopback), "{host}");
    }
    for host in ["attacker.example:5555", "192.168.1.10:5555", ""] {
        assert!(!host_is_allowed(Some(host), &loopback), "{host}");
    }
    assert!(!host_is_allowed(None, &loopback));

    // Bound to a specific LAN address: that address is Studio's name.
    let lan = StudioConfig {
        host: "192.168.1.10".into(),
        ..Default::default()
    };
    assert!(host_is_allowed(Some("192.168.1.10:5555"), &lan));
    assert!(!host_is_allowed(Some("attacker.example"), &lan));

    // Bound to every interface: clients may use any name only with --yes-i-know.
    let wildcard = StudioConfig {
        host: "0.0.0.0".into(),
        ..Default::default()
    };
    assert!(!host_is_allowed(Some("studio.lan:5555"), &wildcard));
    let trusted = StudioConfig {
        yes_i_know: true,
        ..wildcard
    };
    assert!(host_is_allowed(Some("studio.lan:5555"), &trusted));
}

// ---- K2: read-only mode is enforced by the database, not by the first keyword ----

fn read_only_app(pool: ruprizzle::Pool) -> axum::Router {
    create_router(Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        Some(pool),
    )))
}

async fn post_form(app: axum::Router, uri: &str, sql: &str) -> String {
    let mut body = String::from("sql=");
    for byte in sql.bytes() {
        if byte.is_ascii_alphanumeric() {
            body.push(char::from(byte));
        } else {
            body.push_str(&format!("%{byte:02X}"));
        }
    }
    let req = studio_request()
        .method("POST")
        .header("origin", ORIGIN)
        .header(TOKEN_HEADER, TOKEN)
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
async fn a_read_keyword_cannot_smuggle_a_write_past_read_only_mode_on_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    let app = read_only_app(pool.clone());

    // SQLite runs every `;`-separated statement in one call, so this used to
    // pass the first-keyword check and then delete.
    let body = post_form(
        app.clone(),
        "/studio/sandbox/execute",
        "SELECT 1; DELETE FROM \"users\"",
    )
    .await;
    assert!(body.contains("Query failed"), "{body}");
    assert_eq!(user_count(&pool).await, "2");

    // The same smuggling through the EXPLAIN endpoint.
    let _ = post_form(
        app.clone(),
        "/studio/explain",
        "SELECT 1; DELETE FROM \"users\"",
    )
    .await;
    assert_eq!(user_count(&pool).await, "2");

    // Reads still work in read-only mode.
    let body = post_form(
        app,
        "/studio/sandbox/execute",
        "SELECT email FROM \"users\" ORDER BY id",
    )
    .await;
    assert!(body.contains("alice@example.com"), "{body}");
}

#[tokio::test]
async fn explain_refuses_analyze_in_every_spelling() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    // Even with --allow-writes, a plan request must never execute the statement.
    let app = writable_app(pool.clone());

    for sql in [
        "ANALYZE DELETE FROM \"users\"",
        "analyse DELETE FROM \"users\"",
        "(ANALYZE) DELETE FROM \"users\"",
    ] {
        let body = post_form(app.clone(), "/studio/explain", sql).await;
        assert!(
            body.contains("EXPLAIN ANALYZE executes the statement"),
            "{sql}: {body}"
        );
    }
    assert_eq!(user_count(&pool).await, "2");
}

/// A Postgres pool from `RUPRIZZLE_TEST_PG_URL`, or `None` to skip. With
/// `RUPRIZZLE_REQUIRE_DB` set, an unreachable database fails the test instead.
async fn pg_pool() -> Option<ruprizzle::Pool> {
    let Ok(url) = std::env::var("RUPRIZZLE_TEST_PG_URL") else {
        assert!(
            std::env::var_os("RUPRIZZLE_REQUIRE_DB").is_none(),
            "RUPRIZZLE_REQUIRE_DB is set but RUPRIZZLE_TEST_PG_URL is not"
        );
        return None;
    };
    match ruprizzle::connect(&url).await {
        Ok(pool) => Some(pool),
        Err(e) if std::env::var_os("RUPRIZZLE_REQUIRE_DB").is_some() => {
            panic!("Postgres unreachable: {e}")
        }
        Err(_) => None,
    }
}

async fn pg_exec(pool: &ruprizzle::Pool, sql: String) {
    ruprizzle::Executor::execute_raw(pool, std::borrow::Cow::Owned(sql), Vec::new())
        .await
        .expect("setup statement must run");
}

async fn pg_count(pool: &ruprizzle::Pool, table: &str) -> i64 {
    let batch = ruprizzle::Executor::fetch_all_raw(
        pool,
        std::borrow::Cow::Owned(format!("SELECT count(*) FROM {table}")),
        Vec::new(),
    )
    .await
    .expect("count must run");
    match batch {
        ruprizzle::RowBatch::Postgres(rows) => {
            use ruprizzle::sqlx::Row as _;
            rows[0].get::<i64, _>(0)
        }
        other => panic!("unexpected batch: {other:?}"),
    }
}

#[tokio::test]
async fn read_only_mode_holds_against_postgres_write_forms() {
    let Some(pool) = pg_pool().await else {
        eprintln!("skipped: RUPRIZZLE_TEST_PG_URL not set");
        return;
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let t = format!("studio_k2_{nanos}");
    pg_exec(
        &pool,
        format!("CREATE TABLE {t} (id SERIAL PRIMARY KEY, v TEXT)"),
    )
    .await;
    pg_exec(&pool, format!("INSERT INTO {t} (v) VALUES ('a'), ('b')")).await;

    let app = read_only_app(pool.clone());
    for sql in [
        // EXPLAIN ANALYZE executes the statement it explains.
        format!("EXPLAIN ANALYZE DELETE FROM {t}"),
        // A data-modifying CTE behind a SELECT.
        format!("WITH d AS (DELETE FROM {t} RETURNING *) SELECT count(*) FROM d"),
        // DDL and sequence mutation dressed as SELECT.
        format!("SELECT * INTO {t}_copy FROM {t}"),
        format!("SELECT setval('{t}_id_seq', 1000)"),
    ] {
        let body = post_form(app.clone(), "/studio/sandbox/execute", &sql).await;
        assert!(body.contains("read-only transaction"), "{sql}: {body}");
    }
    let body = post_form(
        app.clone(),
        "/studio/explain",
        &format!("ANALYZE DELETE FROM {t}"),
    )
    .await;
    assert!(
        body.contains("EXPLAIN ANALYZE executes the statement"),
        "{body}"
    );

    assert_eq!(pg_count(&pool, &t).await, 2, "no row may be deleted");
    let body = post_form(
        app,
        "/studio/sandbox/execute",
        &format!("SELECT v FROM {t} ORDER BY id"),
    )
    .await;
    assert!(body.contains("2 row(s)"), "reads still work: {body}");

    pg_exec(&pool, format!("DROP TABLE IF EXISTS {t}_copy")).await;
    pg_exec(&pool, format!("DROP TABLE {t}")).await;
}

#[tokio::test]
async fn the_schema_graph_draws_each_relation_once_between_its_key_columns() {
    let state = Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        None,
    ));
    let body = get_body(create_router(state), "/studio/erd/data").await;
    let graph: serde_json::Value = serde_json::from_str(&body).expect("ERD data must be JSON");

    // `User.posts` and `Post.author` are two sides of one relation: one edge.
    let relations = graph["relations"].as_array().unwrap();
    assert_eq!(relations.len(), 1, "{body}");
    let edge = &relations[0];
    assert_eq!(edge["from"], "Post");
    assert_eq!(edge["to"], "User");
    assert_eq!(edge["from_fields"], serde_json::json!(["authorId"]));
    assert_eq!(edge["to_fields"], serde_json::json!(["id"]));
    assert_eq!(edge["kind"], "many_to_one");

    let post = graph["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Post")
        .unwrap();
    let author_id = post["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "authorId")
        .unwrap();
    assert_eq!(author_id["is_fk"], true);
    // The DSL spelling, not the IR's `Debug` output.
    assert_eq!(author_id["type_name"], "Int");
    assert!(!body.contains("Scalar("), "{body}");
}

#[tokio::test]
async fn studio_serves_the_scripts_its_pages_load() {
    let state = Arc::new(AppState::new(
        sample_schema(),
        StudioConfig::default(),
        None,
    ));
    let app = create_router(state);

    let erd_page = get_body(app.clone(), "/studio/erd").await;
    for script in ["htmx.min.js", "studio.js", "erd.js"] {
        let uri = format!("/studio/assets/js/{script}");
        if script != "erd.js" {
            assert!(
                erd_page.contains(&uri),
                "{script} is not loaded by the page"
            );
        }
        let body = get_body(app.clone(), &uri).await;
        assert!(body.len() > 1000, "{script} is missing or a stub");
    }
    assert!(erd_page.contains("/studio/assets/js/erd.js"));
    // The real htmx, not a hand-written subset that ignored `hx-headers`.
    let htmx = get_body(app, "/studio/assets/js/htmx.min.js").await;
    assert!(
        htmx.contains("hx-headers"),
        "vendored htmx is not the real library"
    );
}

#[tokio::test]
async fn explain_answers_with_a_fragment_not_a_second_page() {
    let dir = tempfile::tempdir().unwrap();
    let app = writable_app(seeded_pool(dir.path()).await);

    let body = post_form(app, "/studio/explain", "SELECT * FROM \"users\"").await;
    assert!(body.contains("Query plan"), "{body}");
    // The sandbox swaps this into its result card; a page shell here nested a
    // second sidebar inside it.
    assert!(!body.contains("<html"), "{body}");
    assert!(!body.contains("sidebar"), "{body}");
}

#[tokio::test]
async fn table_cells_keep_database_text_out_of_scripts() {
    let dir = tempfile::tempdir().unwrap();
    let pool = seeded_pool(dir.path()).await;
    ruprizzle::Executor::execute_raw(
        &pool,
        std::borrow::Cow::Borrowed(
            "INSERT INTO \"users\" (id, email, name) VALUES (3, 'x@example.com', '''+alert(1)+''<b>')",
        ),
        Vec::new(),
    )
    .await
    .unwrap();
    let body = get_body(writable_app(pool), "/studio/models/User").await;

    // Cell values used to be spliced into `x-data="{ value: '…' }"`, which the
    // page evaluated as JavaScript.
    assert!(!body.contains("x-data"), "{body}");
    assert!(!body.contains("<b>"), "{body}");
    // Editable cells carry the value as an escaped attribute...
    assert!(body.contains("data-column=\"name\""), "{body}");
    assert!(body.contains("rows/3/cell?column=name"), "{body}");
    // ...but a primary key is never offered for inline editing.
    assert!(!body.contains("cell?column=id"), "{body}");
}

#[tokio::test]
async fn the_grid_only_offers_a_next_page_when_one_can_exist() {
    let dir = tempfile::tempdir().unwrap();
    let app = writable_app(seeded_pool(dir.path()).await);

    let body = get_body(app, "/studio/models/User/table?search=bob").await;
    assert!(body.contains("bob@example.com"), "{body}");
    // Paging keeps the active search.
    assert!(body.contains("hx-include=\"#table-search\""), "{body}");
    let next = body
        .split("Next →")
        .next()
        .and_then(|head| head.rsplit("<button").next())
        .unwrap();
    assert!(
        next.contains("disabled"),
        "Next is live on the last page: {next}"
    );
}
