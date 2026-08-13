//! Soak/smoke test: sustained mixed load with connection churn.
//!
//! The default run is short (30 s) so it can act as a smoke test in CI.
//! Set `RUPRIZZLE_SOAK_DURATION_SECONDS` to run for longer; the W4 exit-gate
//! target is 48 hours.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ruprizzle::executor::Executor;
use ruprizzle_testkit::{both_dbs, TestDb};
use tokio::time::{Instant, interval};

fn soak_duration() -> Duration {
    let secs: u64 = std::env::var("RUPRIZZLE_SOAK_DURATION_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

fn workers() -> usize {
    std::env::var("RUPRIZZLE_SOAK_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

fn report_pool_health(db: &TestDb, elapsed: Duration, ops: u64, errors: u64) {
    let stats = ruprizzle::pool::report_metrics(db.pool());
    let waiters = db.pool().num_waiters();
    let mem = simple_process_stats::ProcessStats::get()
        .map(|s| s.memory_usage_bytes)
        .unwrap_or(0);
    eprintln!(
        "soak health: elapsed={elapsed:?} ops={ops} errors={errors} size={} idle={} in_use={} waiters={waiters} memory_bytes={mem}",
        stats.size, stats.idle, stats.in_use
    );
    let _ = stats;
    let _ = waiters;
    let _ = (ops, errors);
}

async fn mixed_load(db: TestDb) -> ruprizzle_testkit::Result {
    let db = Arc::new(db);
    let duration = soak_duration();
    let workers = workers();
    let pool = db.pool().clone();
    let ops = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));

    pool.execute_raw(
        "CREATE TABLE IF NOT EXISTS soak_kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)"
            .to_owned()
            .into(),
        Vec::new(),
    )
    .await?;

    let start = Instant::now();
    let mut health = interval(Duration::from_secs(5));

    let mut handles = Vec::new();
    for w in 0..workers {
        let pool = pool.clone();
        let ops = Arc::clone(&ops);
        let errors = Arc::clone(&errors);
        handles.push(tokio::spawn(async move {
            let mut local = 0u64;
            while start.elapsed() < duration {
                local += 1;
                let key = format!("w{w}-k{local}");
                let value = format!("v-{local}");
                let sql = match local % 4 {
                    0 => "INSERT INTO soak_kv (k, v) VALUES ($1, $2)",
                    1 => "UPDATE soak_kv SET v = $2 WHERE k = $1",
                    2 => "SELECT v FROM soak_kv WHERE k = $1",
                    _ => "DELETE FROM soak_kv WHERE k = $1",
                };
                let binds = vec![
                    ruprizzle::Encodable::to_value(&key),
                    ruprizzle::Encodable::to_value(&value),
                ];
                if let Err(_) = pool.execute_raw(sql.to_owned().into(), binds).await {
                    errors.fetch_add(1, Ordering::Relaxed);
                }
                ops.fetch_add(1, Ordering::Relaxed);

                // Connection churn: some percentage of work is done inside a
                // transaction so the pool sees frequent acquire/release cycles.
                if local % 7 == 0 {
                    if let Ok(tx) = pool.begin().await {
                        let _ = tx
                            .execute_raw(
                                "SELECT COUNT(*) FROM soak_kv".to_owned().into(),
                                Vec::new(),
                            )
                            .await;
                        let _ = tx.commit().await;
                    }
                }
            }
        }));
    }

    let reporter = tokio::spawn({
        let db = Arc::clone(&db);
        let ops = Arc::clone(&ops);
        let errors = Arc::clone(&errors);
        async move {
            while start.elapsed() < duration {
                health.tick().await;
                report_pool_health(&db, start.elapsed(), ops.load(Ordering::Relaxed), errors.load(Ordering::Relaxed));
            }
        }
    });

    for h in handles {
        h.await.expect("worker panic");
    }
    reporter.abort();

    let total_ops = ops.load(Ordering::Relaxed);
    let total_errors = errors.load(Ordering::Relaxed);
    report_pool_health(&db, duration, total_ops, total_errors);

    // Consistency: no errors (we don't expect contention on different keys),
    // and the table should be reachable.
    assert_eq!(total_errors, 0, "soak produced errors");
    let count = db.fetch_i64("SELECT COUNT(*) FROM soak_kv").await?;
    eprintln!("soak finished: {total_ops} operations, {count} rows remaining");

    Ok(())
}

both_dbs! {
    setup = "CREATE TABLE IF NOT EXISTS soak_kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)";
    async fn soak_mixed_load_with_connection_churn(db: TestDb) {
        mixed_load(db).await?;
    }
}
