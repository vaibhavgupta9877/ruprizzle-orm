//! Slow-query warnings.

use std::fmt;
use std::sync::{Arc, Mutex};

use ruprizzle::{Executor, PoolConfig};
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

#[test]
fn slow_query_emits_warn_with_sql_shape_and_bind_count() {
    let captured = Captured::default();
    let events = captured.0.clone();
    run_with_capture(captured, async {
        let mut config = PoolConfig::default();
        // Any query taking longer than zero is a slow query in this test.
        config.slow_query_threshold = Some(std::time::Duration::from_nanos(1));
        let pool = ruprizzle::connect_with("sqlite::memory:", &config)
            .await
            .expect("connect");
        pool.execute_raw("CREATE TABLE t (id INTEGER)".to_owned().into(), Vec::new())
            .await
            .expect("create table");
    });

    let events = events.lock().expect("capture lock");
    let slow = events
        .iter()
        .find(|e| e.target == "ruprizzle::slow_query")
        .expect("slow query event");
    assert_eq!(slow.level, "WARN");
    assert!(field(slow, "sql").is_some());
    assert_eq!(field(slow, "binds"), Some("0"));
    assert!(field(slow, "elapsed_ms").is_some());
}

/// The native rusqlite read path decodes rows straight from the driver, outside
/// the executor's `fetch_all_raw`. It must still be observed: a regression left
/// every rusqlite `SELECT` without a `db.query` span, metrics, or slow-query
/// warning.
#[cfg(feature = "sqlite-rusqlite")]
mod rusqlite_reads {
    use super::*;
    use ruprizzle::{Model, SelectQuery};

    #[derive(Debug, Clone, Default, sqlx::FromRow)]
    struct Item {
        id: i64,
    }

    impl Model for Item {
        const TABLE: &'static str = "items";
        const PRIMARY_KEY: &'static str = "id";
    }

    #[cfg(feature = "postgres-tokio-postgres")]
    ruprizzle::tokio_postgres_default_row!(Item);

    impl ruprizzle::rusqlite::FromRusqliteRow for Item {
        fn from_rusqlite_row(
            row: &ruprizzle::rusqlite::RusqliteRow,
        ) -> Result<Self, ruprizzle::Error> {
            Ok(Self {
                id: ruprizzle::rusqlite::get_i64(row, 0)?,
            })
        }
    }

    impl ruprizzle::rusqlite::FromOwnedRow for Item {
        fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
            Ok(Self {
                id: row.get::<i64>(0)?,
            })
        }
    }

    async fn seeded_pool() -> ruprizzle::Pool {
        let mut config = PoolConfig::default();
        config.slow_query_threshold = Some(std::time::Duration::from_nanos(1));
        let pool = ruprizzle::connect_with("sqlite::memory:?driver=rusqlite", &config)
            .await
            .expect("connect");
        pool.execute_raw(
            "CREATE TABLE items (id INTEGER PRIMARY KEY)"
                .to_owned()
                .into(),
            Vec::new(),
        )
        .await
        .expect("create");
        pool.execute_raw(
            "INSERT INTO items (id) VALUES (7)".to_owned().into(),
            Vec::new(),
        )
        .await
        .expect("insert");
        pool
    }

    fn slow_selects(events: &[EventRecord]) -> usize {
        events
            .iter()
            .filter(|e| e.target == "ruprizzle::slow_query")
            .filter(|e| field(e, "sql").is_some_and(|s| s.contains("SELECT")))
            .count()
    }

    #[test]
    fn async_rusqlite_select_is_observed() {
        let captured = Captured::default();
        let events = captured.0.clone();
        run_with_capture(captured, async {
            let pool = seeded_pool().await;
            let rows = SelectQuery::<Item>::new(&pool)
                .fetch_all()
                .await
                .expect("select");
            assert_eq!(rows[0].id, 7);
        });
        assert_eq!(slow_selects(&events.lock().expect("lock")), 1);
    }

    #[test]
    fn blocking_rusqlite_select_is_observed() {
        let captured = Captured::default();
        let events = captured.0.clone();
        run_with_capture(captured, async {
            let pool = seeded_pool().await;
            let rows = SelectQuery::<Item>::new(&pool)
                .fetch_all_blocking()
                .expect("select");
            assert_eq!(rows[0].id, 7);
            assert_eq!(
                SelectQuery::<Item>::new(&pool)
                    .count_blocking()
                    .expect("count"),
                1
            );
        });
        assert_eq!(slow_selects(&events.lock().expect("lock")), 2);
    }
}
