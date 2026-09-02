//! End-to-end coverage of the Hrana HTTP adapter against a local server.
//!
//! These tests assert both halves of the round trip that unit tests cannot: that the
//! adapter posts a well-formed `/v2/pipeline` request with the bearer token and the
//! bound parameters, and that it turns the server's answer into rows, counts and
//! errors. The server is a few dozen lines of `tokio` rather than a mocking crate,
//! so the bytes on the wire are the thing under test.

use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use ruprizzle::Executor;
use ruprizzle::executor::RowBatch;
use ruprizzle::value::Value;
use ruprizzle_turso::{TursoError, TursoPool};
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

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn pool(&self) -> TursoPool {
        TursoPool::connect(&self.url(), "test-token").expect("the pool builds")
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

const TWO_ROWS: &str = r#"{"baton":null,"base_url":null,"results":[
    {"type":"ok","response":{"type":"execute","result":{
        "cols":[{"name":"id"},{"name":"email"}],
        "rows":[[{"type":"integer","value":"7"},{"type":"text","value":"a@b.c"}],
                [{"type":"integer","value":"8"},{"type":"null"}]],
        "affected_row_count":0,"last_insert_rowid":null}}},
    {"type":"ok","response":{"type":"close"}}]}"#;

#[tokio::test]
async fn a_select_is_posted_as_a_pipeline_and_comes_back_as_rows() {
    let server = MockServer::spawn(200, TWO_ROWS, 1).await;
    let pool = server.pool();

    let batch = pool
        .fetch_all_raw(
            Cow::Borrowed("SELECT id, email FROM users WHERE id > ?"),
            vec![Value::I64(6)],
        )
        .await
        .expect("the query succeeds");

    let request = server.first_request();
    assert_eq!(request.target, "/v2/pipeline");
    assert_eq!(request.authorization.as_deref(), Some("Bearer test-token"));

    let sent: serde_json::Value = serde_json::from_str(&request.body).expect("valid JSON body");
    assert_eq!(
        sent["requests"][0]["stmt"]["sql"],
        "SELECT id, email FROM users WHERE id > ?"
    );
    assert_eq!(sent["requests"][0]["stmt"]["args"][0]["type"], "integer");
    assert_eq!(sent["requests"][0]["stmt"]["args"][0]["value"], "6");
    assert_eq!(
        sent["requests"][1]["type"], "close",
        "the stream must be closed so the server does not hold a session"
    );

    let RowBatch::Edge(rows) = batch else {
        panic!("the Turso adapter produces edge rows");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get("id"), Some(&Value::I64(7)));
    assert_eq!(rows[0].get("email"), Some(&Value::Str("a@b.c".into())));
    assert_eq!(rows[1].get("email"), Some(&Value::Null));
}

#[tokio::test]
async fn a_write_reports_the_row_count_the_server_sent() {
    let body = r#"{"baton":null,"results":[
        {"type":"ok","response":{"type":"execute","result":{
            "cols":[],"rows":[],"affected_row_count":3}}},
        {"type":"ok","response":{"type":"close"}}]}"#;
    let server = MockServer::spawn(200, body, 1).await;

    let affected = server
        .pool()
        .execute_raw(
            Cow::Borrowed("UPDATE users SET active = ? WHERE id > ?"),
            vec![Value::Bool(true), Value::I64(4)],
        )
        .await
        .expect("the statement succeeds");

    assert_eq!(affected, 3);

    let sent: serde_json::Value =
        serde_json::from_str(&server.first_request().body).expect("valid JSON body");
    assert_eq!(
        sent["requests"][0]["stmt"]["want_rows"], false,
        "a write must not ask the server to ship a result set"
    );
    assert_eq!(
        sent["requests"][0]["stmt"]["args"][0]["value"], "1",
        "SQLite stores booleans as 1 and 0"
    );
}

#[tokio::test]
async fn a_rejected_statement_surfaces_the_servers_message() {
    let body = r#"{"baton":null,"results":[
        {"type":"error","error":{"message":"no such table: users","code":"SQLITE_UNKNOWN"}}]}"#;
    let server = MockServer::spawn(200, body, 1).await;

    let err = server
        .pool()
        .fetch_all_raw(Cow::Borrowed("SELECT * FROM users"), Vec::new())
        .await
        .expect_err("the server rejected the statement");

    let rendered = err.to_string();
    assert!(rendered.contains("no such table: users"), "{rendered}");
    assert!(rendered.contains("SQLITE_UNKNOWN"), "{rendered}");
}

#[tokio::test]
async fn an_http_failure_is_reported_rather_than_read_as_an_empty_result() {
    let server = MockServer::spawn(401, r#"{"error":"unauthorized"}"#, 1).await;

    let err = server
        .pool()
        .fetch_all_raw(Cow::Borrowed("SELECT 1"), Vec::new())
        .await
        .expect_err("401 is not a result set");

    assert!(err.to_string().contains("401"), "{err}");
}

#[tokio::test]
async fn a_dead_endpoint_is_a_transport_error() {
    // Bind and drop, so the port is almost certainly closed.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);

    let pool = TursoPool::connect(&format!("http://{addr}"), "t").expect("the pool builds");
    let err = pool
        .fetch_all_raw(Cow::Borrowed("SELECT 1"), Vec::new())
        .await
        .expect_err("nothing is listening");

    assert!(err.to_string().contains("transport"), "{err}");
}

#[tokio::test]
async fn an_unbindable_parameter_never_reaches_the_network() {
    let server = MockServer::spawn(200, TWO_ROWS, 1).await;

    let err = server
        .pool()
        .fetch_all_raw(
            Cow::Borrowed("SELECT * FROM users WHERE tags = ?"),
            vec![Value::Array(vec![Value::I64(1)])],
        )
        .await
        .expect_err("SQLite has no array type");

    assert!(err.to_string().contains("array"), "{err}");
    assert!(
        server.seen.lock().expect("not poisoned").is_empty(),
        "the request must be refused before it is sent"
    );
}

#[test]
fn a_query_error_is_distinguishable_from_a_transport_error() {
    // The variants exist so a caller can retry a transport failure and not retry a
    // rejected statement; this pins that they stay separate.
    let query = TursoError::Query {
        message: "no such table".into(),
        code: None,
    };
    assert!(!matches!(query, TursoError::Transport(_)));
}
