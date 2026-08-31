//! End-to-end coverage of the D1 REST adapter against a local server.
//!
//! These tests assert both halves of the round trip that unit tests cannot: that the
//! adapter posts to the right path with the bearer token and the bound parameters,
//! and that it turns Cloudflare's envelope into rows, counts and errors. The server
//! is a few dozen lines of `tokio` rather than a mocking crate, so the bytes on the
//! wire are the thing under test.

use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use ruprizzle::Executor;
use ruprizzle::executor::RowBatch;
use ruprizzle::value::Value;
use ruprizzle_d1::D1Pool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A one-shot HTTP server that answers with a canned response and keeps what it was
/// asked, so a test can assert on the request the adapter actually built.
struct MockServer {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Request>>>,
}

/// The parts of a request these tests care about.
#[derive(Clone)]
struct Request {
    target: String,
    authorization: Option<String>,
    body: String,
}

impl MockServer {
    /// Starts a server that answers `count` requests with `status` and `body`.
    async fn spawn(status: u16, body: &'static str, count: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&seen);

        tokio::spawn(async move {
            for _ in 0..count {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let request = read_request(&mut socket).await;
                recorder.lock().expect("not poisoned").push(request);

                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });

        Self { addr, seen }
    }

    fn pool(&self) -> D1Pool {
        D1Pool::builder()
            .account_id("acct")
            .database_id("db-uuid")
            .api_token("test-token")
            .endpoint(format!("http://{}", self.addr))
            .build()
            .expect("the pool builds")
    }

    fn first_request(&self) -> Request {
        self.seen
            .lock()
            .expect("not poisoned")
            .first()
            .cloned()
            .expect("the adapter sent a request")
    }
}

/// Reads one HTTP/1.1 request: head, then exactly `Content-Length` body bytes.
async fn read_request(socket: &mut tokio::net::TcpStream) -> Request {
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];

    // Read the head one byte at a time so we never consume past it.
    while !raw.ends_with(b"\r\n\r\n") {
        match socket.read(&mut byte).await {
            Ok(0) | Err(_) => break,
            Ok(_) => raw.push(byte[0]),
        }
    }
    let head = String::from_utf8_lossy(&raw).into_owned();

    let header = |name: &str| {
        head.lines()
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            })
            .map(|(_, v)| v.trim().to_owned())
    };

    let length: usize = header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    if length > 0 {
        socket.read_exact(&mut body).await.expect("read body");
    }

    Request {
        target: head
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or_default()
            .to_owned(),
        authorization: header("authorization"),
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

const ONE_ROW: &str = r#"{"success":true,"errors":[],"messages":[],"result":[{
    "results":[{"id":7,"email":"a@b.c","note":null}],
    "success":true,
    "meta":{"changes":0,"last_row_id":0,"rows_read":1,"rows_written":0}}]}"#;

#[tokio::test]
async fn a_select_is_posted_to_the_query_endpoint_and_comes_back_as_rows() {
    let server = MockServer::spawn(200, ONE_ROW, 1).await;

    let batch = server
        .pool()
        .fetch_all_raw(
            Cow::Borrowed("SELECT id, email, note FROM users WHERE id = ?"),
            vec![Value::I64(7)],
        )
        .await
        .expect("the query succeeds");

    let request = server.first_request();
    assert_eq!(request.target, "/accounts/acct/d1/database/db-uuid/query");
    assert_eq!(request.authorization.as_deref(), Some("Bearer test-token"));

    let sent: serde_json::Value = serde_json::from_str(&request.body).expect("valid JSON body");
    assert_eq!(
        sent["sql"],
        "SELECT id, email, note FROM users WHERE id = ?"
    );
    assert_eq!(sent["params"][0], 7);

    let RowBatch::Edge(rows) = batch else {
        panic!("the D1 adapter produces edge rows");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("id"), Some(&Value::I64(7)));
    assert_eq!(rows[0].get("email"), Some(&Value::Str("a@b.c".into())));
    assert_eq!(rows[0].get("note"), Some(&Value::Null));
}

#[tokio::test]
async fn a_write_reports_the_change_count_from_the_metadata() {
    let body = r#"{"success":true,"errors":[],"messages":[],"result":[{
        "results":[],"success":true,"meta":{"changes":2,"last_row_id":9}}]}"#;
    let server = MockServer::spawn(200, body, 1).await;

    let affected = server
        .pool()
        .execute_raw(
            Cow::Borrowed("UPDATE users SET active = ? WHERE id > ?"),
            vec![Value::Bool(true), Value::I64(4)],
        )
        .await
        .expect("the statement succeeds");

    assert_eq!(affected, 2);

    let sent: serde_json::Value =
        serde_json::from_str(&server.first_request().body).expect("valid JSON body");
    assert_eq!(
        sent["params"][0], 1,
        "SQLite stores booleans as 1 and 0, and D1 refuses a JSON boolean parameter"
    );
}

#[tokio::test]
async fn a_rejected_statement_surfaces_cloudflares_message_and_code() {
    let body = r#"{"success":false,
        "errors":[{"code":7500,"message":"no such table: users"}],
        "messages":[],"result":null}"#;
    let server = MockServer::spawn(400, body, 1).await;

    let err = server
        .pool()
        .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
        .await
        .expect_err("Cloudflare rejected the statement");

    let rendered = err.to_string();
    assert!(rendered.contains("no such table: users"), "{rendered}");
    assert!(rendered.contains("7500"), "{rendered}");
}

#[tokio::test]
async fn a_body_that_is_not_an_envelope_is_reported_with_the_status() {
    let server = MockServer::spawn(502, "<html>bad gateway</html>", 1).await;

    let err = server
        .pool()
        .fetch_all_raw(Cow::Borrowed("SELECT 1"), Vec::new())
        .await
        .expect_err("a gateway error is not a result set");

    assert!(err.to_string().contains("502"), "{err}");
}

#[tokio::test]
async fn a_dead_endpoint_is_a_transport_error() {
    // Bind and drop, so the port is almost certainly closed.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);

    let pool = D1Pool::builder()
        .account_id("a")
        .database_id("b")
        .api_token("t")
        .endpoint(format!("http://{addr}"))
        .build()
        .expect("the pool builds");

    let err = pool
        .fetch_all_raw(Cow::Borrowed("SELECT 1"), Vec::new())
        .await
        .expect_err("nothing is listening");

    assert!(err.to_string().contains("transport"), "{err}");
}

#[tokio::test]
async fn a_binary_parameter_never_reaches_the_network() {
    let server = MockServer::spawn(200, ONE_ROW, 1).await;

    let err = server
        .pool()
        .fetch_all_raw(
            Cow::Borrowed("SELECT * FROM files WHERE blob = ?"),
            vec![Value::Bytes(vec![1, 2, 3].into())],
        )
        .await
        .expect_err("D1's HTTP API takes no binary parameter");

    assert!(err.to_string().contains("binary"), "{err}");
    assert!(
        server.seen.lock().expect("not poisoned").is_empty(),
        "the request must be refused before it is sent"
    );
}
