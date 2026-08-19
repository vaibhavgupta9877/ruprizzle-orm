//! Resumable segmented soak test for the native `rusqlite` backend.
//!
//! This test is designed for machines that cannot stay on for a continuous
//! 48-hour run. It stores progress in the same SQLite database that it stresses,
//! so the process can be stopped and restarted and the run will continue until
//! 48 hours of cumulative elapsed time is reached.
//!
//! The database is placed in the workspace under `local/soak-48h/` (never in a
//! system temp directory or on C:) and is reused across segments. Each segment
//! starts with a fresh `soak_kv` table so key cycling does not collide with rows
//! left by an interrupted segment.
//!
//! Environment variables:
//! - `RUPRIZZLE_SOAK_DB_PATH` — required. Absolute path to the SQLite file,
//!   e.g. `D:/SaaS/rust/ruprizzle-orm/local/soak-48h/soak-rusqlite.db`.
//! - `RUPRIZZLE_SOAK_DURATION_SECONDS` — per-segment duration (default `1800`).
//! - `RUPRIZZLE_SOAK_WORKERS` — concurrent workers (default `8`).
//! - `RUPRIZZLE_SOAK_RESUME` — set to `1` to continue an existing run.
//! - `RUPRIZZLE_SOAK_LOG_DIR` — directory for `soak.log` and `soak.err`
//!   (default `local/soak-48h`).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use ruprizzle::executor::Executor;
use ruprizzle_testkit::{Backend, TestDb};
use simple_process_stats::ProcessStats;
use sqlx::{Row, query};
use tokio::time::{Instant, interval};

const TARGET_CUMULATIVE_SECONDS: f64 = 48.0 * 3600.0;
const STATE_ID: i64 = 1;

fn db_path() -> PathBuf {
    std::env::var("RUPRIZZLE_SOAK_DB_PATH")
        .expect("RUPRIZZLE_SOAK_DB_PATH must be set to a persistent SQLite path")
        .into()
}

fn log_dir() -> PathBuf {
    let dir = std::env::var("RUPRIZZLE_SOAK_LOG_DIR")
        .map_or_else(|_| "local/soak-48h".into(), PathBuf::from);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn soak_log() -> Arc<Mutex<File>> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir().join("soak.log"))
        .expect("open soak.log");
    Arc::new(Mutex::new(file))
}

fn soak_err_log() -> Arc<Mutex<File>> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir().join("soak.err"))
        .expect("open soak.err");
    Arc::new(Mutex::new(file))
}

/// Write to the log file and attempt stderr, but never panic if stderr is broken.
/// This is the fix for the `failed printing to stderr: Insufficient system resources`
/// panic seen in the original continuous 48-hour run.
fn log_line(log: &Mutex<File>, line: &str) {
    let mut file = log.lock().unwrap_or_else(|e| e.into_inner());
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

fn segment_duration() -> Duration {
    let secs: u64 = std::env::var("RUPRIZZLE_SOAK_DURATION_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1800);
    Duration::from_secs(secs)
}

fn workers() -> usize {
    std::env::var("RUPRIZZLE_SOAK_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

fn is_resume() -> bool {
    std::env::var("RUPRIZZLE_SOAK_RESUME")
        .ok()
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
struct State {
    cumulative_elapsed: f64,
    total_ops: i64,
    total_errors: i64,
    completed: bool,
}

async fn load_state(db: &TestDb) -> State {
    let row = query(
        "SELECT cumulative_elapsed_seconds, total_ops, total_errors, completed FROM soak_state WHERE id = ?",
    )
    .bind(STATE_ID)
    .fetch_optional(db.any_pool())
    .await;

    match row {
        Ok(Some(row)) => State {
            cumulative_elapsed: row.try_get::<f64, _>(0).unwrap_or(0.0),
            total_ops: row.try_get::<i64, _>(1).unwrap_or(0),
            total_errors: row.try_get::<i64, _>(2).unwrap_or(0),
            completed: row.try_get::<i32, _>(3).unwrap_or(0) != 0,
        },
        _ => State {
            cumulative_elapsed: 0.0,
            total_ops: 0,
            total_errors: 0,
            completed: false,
        },
    }
}

async fn save_state(db: &TestDb, state: &State) {
    let _ = query(
        "INSERT OR REPLACE INTO soak_state (id, cumulative_elapsed_seconds, total_ops, total_errors, completed) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(STATE_ID)
    .bind(state.cumulative_elapsed)
    .bind(state.total_ops)
    .bind(state.total_errors)
    .bind(if state.completed { 1 } else { 0 })
    .execute(db.any_pool())
    .await;
}

async fn init_state_table(db: &TestDb) {
    let _ = db
        .execute(
            "CREATE TABLE IF NOT EXISTS soak_state (
                id INTEGER PRIMARY KEY,
                cumulative_elapsed_seconds REAL NOT NULL DEFAULT 0,
                total_ops BIGINT NOT NULL DEFAULT 0,
                total_errors BIGINT NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0
            )",
        )
        .await;
}

fn report_pool_health(log: &Mutex<File>, db: &TestDb, elapsed: Duration, ops: u64, errors: u64) {
    let stats = ruprizzle::pool::report_metrics(db.pool());
    let waiters = db.pool().num_waiters();
    let mem = ProcessStats::get()
        .map(|s| s.memory_usage_bytes)
        .unwrap_or(0);
    let line = format!(
        "soak health: elapsed={:?} ops={ops} errors={errors} size={} idle={} in_use={} waiters={waiters} memory_bytes={mem}",
        elapsed, stats.size, stats.idle, stats.in_use
    );
    log_line(log, &line);
}

async fn mixed_load(db: TestDb) -> ruprizzle_testkit::Result {
    let db = Arc::new(db);
    let duration = segment_duration();
    let workers = workers();
    let pool = db.pool().clone();
    let ops = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));

    init_state_table(&db).await;

    let mut state = if is_resume() {
        load_state(&db).await
    } else {
        // Start a fresh run: clear any partial data and the old state.
        let _ = db.execute("DELETE FROM soak_kv").await;
        let _ = db.execute("DELETE FROM soak_state").await;
        save_state(
            &db,
            &State {
                cumulative_elapsed: 0.0,
                total_ops: 0,
                total_errors: 0,
                completed: false,
            },
        )
        .await;
        State {
            cumulative_elapsed: 0.0,
            total_ops: 0,
            total_errors: 0,
            completed: false,
        }
    };

    let log = soak_log();

    if state.completed {
        log_line(
            &log,
            &format!(
                "soak already completed: elapsed={:.3}s ops={} errors={}",
                state.cumulative_elapsed, state.total_ops, state.total_errors
            ),
        );
        return Ok(());
    }

    if state.cumulative_elapsed >= TARGET_CUMULATIVE_SECONDS {
        log_line(
            &log,
            &format!(
                "soak cumulative target already reached: elapsed={:.3}s ops={} errors={}",
                state.cumulative_elapsed, state.total_ops, state.total_errors
            ),
        );
        return Ok(());
    }

    let base_cumulative = state.cumulative_elapsed;

    pool.execute_raw(
        "CREATE TABLE IF NOT EXISTS soak_kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)"
            .to_owned()
            .into(),
        Vec::new(),
    )
    .await?;

    // Start each segment with a clean working set so key cycling does not collide
    // with rows left behind by an interrupted previous segment.
    let _ = db.execute("DELETE FROM soak_kv").await;

    let start = Instant::now();
    let mut health = interval(Duration::from_secs(5));
    let err_log = soak_err_log();

    let mut handles = Vec::new();
    for w in 0..workers {
        let pool = pool.clone();
        let ops = Arc::clone(&ops);
        let errors = Arc::clone(&errors);
        let err_log = Arc::clone(&err_log);
        handles.push(tokio::spawn(async move {
            let mut local = 0u64;
            while start.elapsed() < duration {
                local += 1;
                let cycle = (local - 1) >> 2;
                let op = (local - 1) & 3;
                let key = format!("w{w}-k{cycle}");
                let value = format!("v-{cycle}");
                let (sql, binds) = match op {
                    0 => (
                        "INSERT INTO soak_kv (k, v) VALUES ($1, $2)",
                        vec![
                            ruprizzle::Encodable::to_value(&key),
                            ruprizzle::Encodable::to_value(&value),
                        ],
                    ),
                    1 => (
                        "UPDATE soak_kv SET v = $2 WHERE k = $1",
                        vec![
                            ruprizzle::Encodable::to_value(&key),
                            ruprizzle::Encodable::to_value(&value),
                        ],
                    ),
                    2 => (
                        "SELECT v FROM soak_kv WHERE k = $1",
                        vec![ruprizzle::Encodable::to_value(&key)],
                    ),
                    _ => (
                        "DELETE FROM soak_kv WHERE k = $1",
                        vec![ruprizzle::Encodable::to_value(&key)],
                    ),
                };
                let res = match op {
                    2 => pool
                        .fetch_all_raw(sql.to_owned().into(), binds)
                        .await
                        .map(|_| ()),
                    _ => pool
                        .execute_raw(sql.to_owned().into(), binds)
                        .await
                        .map(|_| ()),
                };
                if let Err(e) = res {
                    log_line(&err_log, &format!("soak op error (worker {w}): {e}"));
                    errors.fetch_add(1, Ordering::Relaxed);
                }
                ops.fetch_add(1, Ordering::Relaxed);

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
        let log = Arc::clone(&log);
        let mut state = state.clone();
        async move {
            while start.elapsed() < duration {
                health.tick().await;
                let segment_elapsed = start.elapsed().as_secs_f64();
                let current_ops = ops.load(Ordering::Relaxed);
                let current_errors = errors.load(Ordering::Relaxed);
                state.cumulative_elapsed = base_cumulative + segment_elapsed;
                state.total_ops = current_ops as i64;
                state.total_errors = current_errors as i64;
                save_state(&db, &state).await;
                report_pool_health(
                    &log,
                    &db,
                    Duration::from_secs_f64(state.cumulative_elapsed),
                    current_ops,
                    current_errors,
                );
            }
        }
    });

    for h in handles {
        h.await.expect("worker panic");
    }
    reporter.abort();

    let segment_elapsed = start.elapsed().as_secs_f64();
    let segment_ops = ops.load(Ordering::Relaxed);
    let segment_errors = errors.load(Ordering::Relaxed);

    // Final state update. If the segment was interrupted before this point, the
    // last 5-second health tick already persisted the state, so only the last few
    // seconds are lost.
    state.cumulative_elapsed = base_cumulative + segment_elapsed;
    state.total_ops = segment_ops as i64;
    state.total_errors = segment_errors as i64;

    let reached_target = state.cumulative_elapsed >= TARGET_CUMULATIVE_SECONDS;
    state.completed = reached_target;
    save_state(&db, &state).await;

    let count = db.fetch_i64("SELECT COUNT(*) FROM soak_kv").await?;
    assert_eq!(state.total_errors, 0, "soak produced errors");
    if reached_target {
        log_line(
            &log,
            &format!(
                "soak finished: cumulative_elapsed={:.3}s ops={} errors={} rows={}",
                state.cumulative_elapsed, state.total_ops, state.total_errors, count
            ),
        );
    } else {
        log_line(
            &log,
            &format!(
                "soak segment finished: cumulative_elapsed={:.3}s ops={} errors={} rows={}; rerun with RUPRIZZLE_SOAK_RESUME=1",
                state.cumulative_elapsed, state.total_ops, state.total_errors, count
            ),
        );
    }

    Ok(())
}

#[tokio::test]
async fn soak_rusqlite_resumable_48h() {
    // Ensure the runner set a workspace-local DB path. If not, the test aborts
    // with a clear message rather than silently using a temp directory on C:.
    let db_path = db_path();
    assert!(
        db_path.is_absolute(),
        "RUPRIZZLE_SOAK_DB_PATH must be an absolute path"
    );

    let db = TestDb::connect(
        Backend::Sqlite,
        "CREATE TABLE IF NOT EXISTS soak_kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)",
    )
    .await
    .expect("connect to persistent SQLite soak database");

    mixed_load(db)
        .await
        .expect("resumable soak should not fail if no errors occurred");
}
