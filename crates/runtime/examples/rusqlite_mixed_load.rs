//! Blocking-pool sizing under mixed read/write load (Enhancements2 §6, the
//! open question that gates Phase C shipping).
//!
//! `enhancements2 §3.2b` measured read-only concurrency. This harness adds
//! writers. It sweeps the number of concurrent client tasks against a fixed
//! native `rusqlite` pool and reports throughput, latency percentiles, and
//! pool-exhaustion errors, for a read-only mix and a 25%-write mix.
//!
//! Run:
//! ```text
//! cargo run --release -p ruprizzle --features sqlite-rusqlite --example rusqlite_mixed_load
//! ```
//! Environment: `RUSQLITE_LOAD_SECS` (default 3) per cell.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ruprizzle::{Executor, PoolConfig, Value, connect_with};

const POOL_SIZE: u32 = 8;
const ROWS: i64 = 1000;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

struct Cell {
    tasks: usize,
    write_pct: u64,
    ops_per_sec: f64,
    p50_us: f64,
    p99_us: f64,
    errors: u64,
}

async fn run_cell(url: &str, tasks: usize, write_pct: u64, secs: u64) -> Cell {
    let mut config = PoolConfig::default();
    config.max_connections = POOL_SIZE;
    config.acquire_timeout = Duration::from_secs(10);
    let pool = connect_with(url, &config).await.expect("connect");

    let stop = Arc::new(AtomicBool::new(false));
    let errors = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    for t in 0..tasks {
        let pool = pool.clone();
        let stop = stop.clone();
        let errors = errors.clone();
        handles.push(tokio::spawn(async move {
            let mut lat = Vec::new();
            let mut i: u64 = t as u64;
            while !stop.load(Ordering::Relaxed) {
                i = i
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let id = ((i >> 33) as i64).rem_euclid(ROWS) + 1;
                let is_write = ((i >> 17) % 100) < write_pct;
                let start = Instant::now();
                let res = if is_write {
                    pool.execute_raw(
                        "UPDATE users SET age = age + 1 WHERE id = ?".into(),
                        vec![Value::I64(id)],
                    )
                    .await
                    .map(|_| ())
                } else {
                    pool.fetch_all_raw(
                        "SELECT id, age FROM users WHERE id = ?".into(),
                        vec![Value::I64(id)],
                    )
                    .await
                    .map(|_| ())
                };
                let us = start.elapsed().as_secs_f64() * 1e6;
                match res {
                    Ok(()) => lat.push(us),
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            lat
        }));
    }

    let window = Instant::now();
    tokio::time::sleep(Duration::from_secs(secs)).await;
    stop.store(true, Ordering::Relaxed);

    let mut all = Vec::new();
    for h in handles {
        all.extend(h.await.expect("task"));
    }
    let elapsed = window.elapsed().as_secs_f64();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());

    Cell {
        tasks,
        write_pct,
        ops_per_sec: all.len() as f64 / elapsed,
        p50_us: percentile(&all, 0.50),
        p99_us: percentile(&all, 0.99),
        errors: errors.load(Ordering::Relaxed),
    }
}

async fn seed(pool: &ruprizzle::Pool) {
    pool.execute_raw(
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, age INTEGER NOT NULL)".into(),
        Vec::new(),
    )
    .await
    .expect("schema");
    pool.execute_raw("DELETE FROM users".into(), Vec::new())
        .await
        .expect("clear");
    for id in 1..=ROWS {
        pool.execute_raw(
            "INSERT INTO users (id, age) VALUES (?, ?)".into(),
            vec![Value::I64(id), Value::I64(20 + id % 50)],
        )
        .await
        .expect("seed row");
    }
}

#[tokio::main]
async fn main() {
    let secs: u64 = std::env::var("RUSQLITE_LOAD_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    let dir = std::env::temp_dir().join("ruprizzle-rusqlite-load");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("load.sqlite");
    let file = path.to_str().unwrap().replace('\\', "/");
    let url = format!("sqlite:///{file}?mode=rwc&driver=rusqlite");
    let mut config = PoolConfig::default();
    config.max_connections = POOL_SIZE;
    let seed_pool = connect_with(&url, &config).await.expect("connect seed");
    seed(&seed_pool).await;
    seed_pool.close().await;

    println!(
        "native rusqlite pool, {POOL_SIZE} connections, {secs}s per cell, {ROWS} rows; \
         blocking threads = tokio default (512)"
    );
    println!(
        "{:>6} {:>8} {:>12} {:>10} {:>10} {:>8}",
        "tasks", "writes%", "ops/s", "p50 us", "p99 us", "errors"
    );
    // `RUSQLITE_LOAD_TASKS=256,1024` overrides the default sweep, for probing
    // past Tokio's blocking-thread limit.
    let tasks_list: Vec<usize> = std::env::var("RUSQLITE_LOAD_TASKS")
        .ok()
        .map(|v| v.split(',').filter_map(|t| t.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![1, 4, 8, 16, 32, 64]);
    for write_pct in [0_u64, 25] {
        for &tasks in &tasks_list {
            let c = run_cell(&url, tasks, write_pct, secs).await;
            println!(
                "{:>6} {:>8} {:>12.0} {:>10.1} {:>10.1} {:>8}",
                c.tasks, c.write_pct, c.ops_per_sec, c.p50_us, c.p99_us, c.errors
            );
        }
    }
}
