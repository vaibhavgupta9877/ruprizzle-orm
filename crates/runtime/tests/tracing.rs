//! Every raw statement must emit a ruprizzle query event.

use std::fmt;
use std::sync::{Arc, Mutex};

use ruprizzle::{Executor, Tx};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;

#[derive(Clone, Debug)]
struct EventRecord {
    level: String,
    fields: Vec<(String, String)>,
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<EventRecord>>>);

#[derive(Default)]
struct FieldVisitor {
    fields: Vec<(String, String)>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields
            .push((field.name().to_owned(), format!("{value:?}")));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .push((field.name().to_owned(), value.to_string()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .push((field.name().to_owned(), value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .push((field.name().to_owned(), value.to_string()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .push((field.name().to_owned(), value.to_owned()));
    }
}

impl<S: Subscriber> Layer<S> for Captured {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() == "ruprizzle::query" {
            let mut visitor = FieldVisitor::default();
            event.record(&mut visitor);
            self.0.lock().expect("capture lock").push(EventRecord {
                level: event.metadata().level().to_string(),
                fields: visitor.fields,
            });
        }
    }
}

fn run_with_capture(captured: Captured, operation: impl std::future::Future<Output = ()>) {
    let subscriber = Registry::default().with(captured);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    tracing::subscriber::with_default(subscriber, || runtime.block_on(operation));
}

fn field<'a>(event: &'a EventRecord, name: &str) -> Option<&'a str> {
    event
        .fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
}

fn event_with_message<'a>(events: &'a [EventRecord], message: &str) -> &'a EventRecord {
    events
        .iter()
        .find(|event| field(event, "message") == Some(message))
        .expect("event with message")
}

#[test]
fn successful_query_emits_a_query_event_with_safe_fields() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:")
            .await
            .expect("connect");
        pool.execute_raw("CREATE TABLE t (id INTEGER)".to_owned().into(), Vec::new())
            .await
            .expect("create table");
    });

    let events = events.lock().expect("capture lock");
    let event = event_with_message(&events, "execute");
    assert_eq!(event.level, "DEBUG");
    assert_eq!(field(event, "binds"), Some("0"));
    assert!(field(event, "sql").is_some());
    assert!(field(event, "rows_affected").is_some());
    assert!(field(event, "elapsed_ms").is_some());
}

#[test]
fn failed_query_emits_a_warn_event_with_an_error_category() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:")
            .await
            .expect("connect");
        let _ = pool
            .execute_raw("THIS IS NOT SQL".to_owned().into(), Vec::new())
            .await;
    });

    let events = events.lock().expect("capture lock");
    let event = event_with_message(&events, "execute failed");
    assert_eq!(event.level, "WARN");
    assert_eq!(field(event, "binds"), Some("0"));
    assert_eq!(field(event, "error"), Some("sqlx"));
    assert!(field(event, "elapsed_ms").is_some());
}

#[test]
fn transaction_execution_and_commit_emit_query_events() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:")
            .await
            .expect("connect");
        let tx = Tx::begin(&pool).await.expect("begin");
        tx.execute_raw("CREATE TABLE t (id INTEGER)".to_owned().into(), Vec::new())
            .await
            .expect("create table");
        tx.commit().await.expect("commit");
    });

    let events = events.lock().expect("capture lock");
    event_with_message(&events, "execute");
    event_with_message(&events, "transaction committed");
}
