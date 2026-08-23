#![cfg(feature = "studio")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ruprizzle_cli::studio::handlers::AppState;
use ruprizzle_cli::studio::{StudioConfig, create_router, is_production_url};
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
async fn test_production_url_guardrail() {
    assert!(is_production_url(
        "postgres://prod-user:pass@prod.db.com/db"
    ));
    assert!(is_production_url(
        "postgresql://user:pass@ep-cool-lake.us-east-2.aws.neon.tech/neondb"
    ));
    assert!(is_production_url("libsql://my-db.turso.io"));
    assert!(!is_production_url("sqlite://dev.db"));
    assert!(!is_production_url(
        "postgres://postgres:postgres@localhost:5432/mydb"
    ));
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
async fn test_studio_table_view_and_grid() {
    let schema = sample_schema();
    let config = StudioConfig {
        allow_writes: false,
        ..Default::default()
    };
    let state = Arc::new(AppState::new(schema, config, None));
    let app = create_router(state);

    // Full table view
    let req = Request::builder()
        .uri("/studio/models/User")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Table grid partial
    let req = Request::builder()
        .uri("/studio/models/User/table")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
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

    // Read-write state
    let read_write_state = Arc::new(AppState::new(
        schema,
        StudioConfig {
            allow_writes: true,
            ..Default::default()
        },
        None,
    ));
    let read_write_app = create_router(read_write_state);

    let req = Request::builder()
        .method("POST")
        .uri("/studio/models/User/rows")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("name=Alice&email=alice@example.com"))
        .unwrap();
    let res = read_write_app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
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
