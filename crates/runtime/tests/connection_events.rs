//! Connection lifecycle tracing.

use std::fmt;
use std::sync::{Arc, Mutex};

use ruprizzle::Executor;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;

#[derive(Clone, Debug)]
struct EventRecord {
    target: String,
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
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.0.lock().expect("capture lock").push(EventRecord {
            target: event.metadata().target().to_owned(),
            level: event.metadata().level().to_string(),
            fields: visitor.fields,
        });
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

fn events_with_target<'a>(events: &'a [EventRecord], target: &str) -> Vec<&'a EventRecord> {
    events.iter().filter(|e| e.target == target).collect()
}

#[test]
fn connect_and_disconnect_emitted_for_sqlite() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:")
            .await
            .expect("connect");
        // A query forces at least one connection to be opened (after_connect).
        pool.execute_raw("SELECT 1".to_owned().into(), Vec::new())
            .await
            .expect("select");
        pool.close().await;
    });

    let events = events.lock().expect("capture lock");
    let conn_events = events_with_target(&events, "ruprizzle::connection");
    let connect = conn_events
        .iter()
        .find(|e| field(e, "event").is_some_and(|v| v == "connect"))
        .expect("connect event");
    assert_eq!(connect.level, "INFO");

    let disconnect = conn_events
        .iter()
        .find(|e| field(e, "event").is_some_and(|v| v == "disconnect"))
        .expect("disconnect event");
    assert_eq!(disconnect.level, "INFO");
}
