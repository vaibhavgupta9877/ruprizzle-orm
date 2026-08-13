//! Concurrency and throughput benchmarks.
//!
//! Covers the axis the end-to-end suite does not: queries per second against
//! pool size, tail latency under contention, and behaviour at pool exhaustion.
//!
//! These benches use SQLite in-memory so `cargo bench` works offline. The
//! numbers are relative; they show how ruprizzle scales with `max_connections`
//! and how it behaves when the pool is fully saturated.

#![forbid(unsafe_code)]

use criterion::{Criterion, criterion_group, criterion_main};
use ruprizzle::{Executor, PoolConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::{Instant, timeout};

const QUERY: &str = "SELECT 1";
const ITERATIONS: usize = 1_000;

fn setup(runtime: &Runtime, config: &PoolConfig) -> Arc<ruprizzle::Pool> {
    let pool = runtime
        .block_on(ruprizzle::connect_with("sqlite::memory:", config))
        .expect("connect");
    runtime
        .block_on(pool.execute_raw(QUERY.to_owned().into(), Vec::new()))
        .expect("warm up");
    Arc::new(pool)
}

fn bench_queries_per_second(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");

    let mut group = c.benchmark_group("concurrency");
    group.measurement_time(Duration::from_secs(5));

    for max_connections in [1, 2, 4, 8] {
        let mut config = PoolConfig::default();
        config.max_connections = max_connections;
        config.min_connections = max_connections;
        let pool = setup(&runtime, &config);

        group.bench_function(format!("qps_pool_size_{max_connections}"), |b| {
            b.iter(|| {
                runtime.block_on(async {
                    let mut handles = Vec::new();
                    for _ in 0..ITERATIONS {
                        let pool = Arc::clone(&pool);
                        handles.push(tokio::spawn(async move {
                            let _ = pool
                                .execute_raw(QUERY.to_owned().into(), Vec::new())
                                .await
                                .expect("query");
                        }));
                    }
                    for h in handles {
                        let _ = h.await;
                    }
                });
            });
        });
    }

    group.finish();
}

fn bench_tail_latency_under_contention(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let mut config = PoolConfig::default();
    config.max_connections = 2;
    config.min_connections = 2;
    // Short timeout so we can observe acquire timeouts without hanging.
    config.acquire_timeout = Duration::from_millis(50);
    let pool = setup(&runtime, &config);

    let mut group = c.benchmark_group("concurrency");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("tail_latency_2_connections_100_concurrent", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let start = Instant::now();
                let mut handles = Vec::new();
                for _ in 0..100 {
                    let pool = Arc::clone(&pool);
                    handles.push(tokio::spawn(async move {
                        let _ = pool
                            .execute_raw(QUERY.to_owned().into(), Vec::new())
                            .await;
                    }));
                }
                for h in handles {
                    let _ = h.await;
                }
                start.elapsed()
            });
        });
    });

    group.finish();
}

fn bench_pool_exhaustion(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let mut config = PoolConfig::default();
    config.max_connections = 1;
    config.min_connections = 1;
    config.acquire_timeout = Duration::from_millis(10);
    let pool = setup(&runtime, &config);

    let mut group = c.benchmark_group("concurrency");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("pool_exhaustion_recovers", |b| {
        b.iter(|| {
            runtime.block_on(async {
                // Hold the only connection open while the pool is drained.
                let guard = pool.begin().await.expect("begin");

                let mut handles = Vec::new();
                for _ in 0..10 {
                    let pool = Arc::clone(&pool);
                    handles.push(tokio::spawn(async move {
                        let _ = timeout(
                            Duration::from_millis(20),
                            pool.execute_raw(QUERY.to_owned().into(), Vec::new()),
                        )
                        .await;
                    }));
                }

                // Release the connection so the queued work can drain.
                drop(guard);

                for h in handles {
                    let _ = h.await;
                }
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_queries_per_second,
    bench_tail_latency_under_contention,
    bench_pool_exhaustion
);
criterion_main!(benches);
