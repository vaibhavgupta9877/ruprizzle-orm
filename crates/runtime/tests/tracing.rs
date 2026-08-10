//! Every raw statement must emit a ruprizzle query event.

use std::sync::{Arc, Mutex};

use ruprizzle::Executor;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<String>>>);

impl<S: Subscriber> Layer<S> for Captured {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() == "ruprizzle::query" {
            self.0
                .lock()
                .expect("capture lock")
                .push(event.metadata().name().to_owned());
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

#[test]
fn successful_query_emits_a_query_event() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:")
            .await
            .expect("connect");
        pool.execute_raw("CREATE TABLE t (id INTEGER)".to_owned(), Vec::new())
            .await
            .expect("create table");
    });
    assert!(!events.lock().expect("capture lock").is_empty());
}

#[test]
fn failed_query_emits_a_query_event() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let pool = ruprizzle::connect("sqlite::memory:")
            .await
            .expect("connect");
        let _ = pool
            .execute_raw("THIS IS NOT SQL".to_owned(), Vec::new())
            .await;
    });
    assert!(!events.lock().expect("capture lock").is_empty());
}
