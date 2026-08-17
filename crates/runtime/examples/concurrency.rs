//! Task C-0: does the `rusqlite` advantage survive concurrency?
//!
//! `blocking_floor` measures one caller at a time and finds `rusqlite` via
//! `spawn_blocking` 13x faster than `sqlx-sqlite`. Both designs end up bounded
//! by SQLite's own locking under load, so the single-threaded number could
//! collapse — and Phase C of the enhancement plan is only worth doing if it
//! does not.
//!
//! Runs the same read workload at 1, 2, 4, 8 and 16 concurrent tasks and
//! reports total throughput (queries/second), not per-query latency: under
//! concurrency, latency inflates by design and throughput is the honest metric.
//!
//! Both arms open the database read-only, so SQLite's shared-cache and WAL
//! writer locks are out of the picture; this measures the driver, not SQLite's
//! write serialisation.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use sqlx::FromRow;

const REPEATS: usize = 3;

/// The cross-ORM benchmark database, as an absolute path.
///
/// Defaults to the checked-in benchmark database relative to the workspace
/// root; override with `RUPRIZZLE_BENCH_DB`.
fn db_path() -> String {
    let raw = std::env::var("RUPRIZZLE_BENCH_DB")
        .unwrap_or_else(|_| "local/cross-orm-bench/node/bench.sqlite3".to_owned());
    let Ok(abs) = std::fs::canonicalize(&raw) else {
        return raw;
    };
    // `canonicalize` yields a Windows extended-length path (`\\?\D:\...`);
    // SQLite wants forward slashes and no prefix.
    let abs = abs.to_string_lossy().replace('\\', "/");
    abs.strip_prefix("//?/").unwrap_or(&abs).to_owned()
}

#[derive(Debug, Clone, FromRow)]
struct User {
    id: i64,
    email: String,
    age: i64,
}

fn open() -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(
        db_path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap()
}

/// A fixed-size set of `rusqlite` connections handed out round-robin.
///
/// Each is behind its own mutex, so `spawn_blocking` tasks contend only when
/// concurrency exceeds the connection count — the same property `sqlx`'s pool
/// has.
struct RusqlitePool {
    conns: Vec<Arc<std::sync::Mutex<rusqlite::Connection>>>,
    next: std::sync::atomic::AtomicUsize,
}

impl RusqlitePool {
    fn new(n: usize) -> Self {
        Self {
            conns: (0..n)
                .map(|_| Arc::new(std::sync::Mutex::new(open())))
                .collect(),
            next: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn get(&self) -> Arc<std::sync::Mutex<rusqlite::Connection>> {
        let i = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Arc::clone(&self.conns[i % self.conns.len()])
    }

    async fn fetch(&self, sql: &'static str) -> usize {
        let conn = self.get();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare_cached(sql).unwrap();
            stmt.query_map([], |r| {
                Ok(User {
                    id: r.get(0)?,
                    email: r.get(1)?,
                    age: r.get(2)?,
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len()
        })
        .await
        .unwrap()
    }
}

/// Total queries per second across `tasks` concurrent workers.
async fn throughput<F, Fut>(tasks: usize, per_task: usize, f: F) -> f64
where
    F: Fn() -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = usize> + Send,
{
    let start = Instant::now();
    let mut handles = Vec::with_capacity(tasks);
    for _ in 0..tasks {
        let f = f.clone();
        handles.push(tokio::spawn(async move {
            let mut n = 0usize;
            for _ in 0..per_task {
                n += f().await;
            }
            n
        }));
    }
    let mut checksum = 0usize;
    for h in handles {
        checksum += h.await.unwrap();
    }
    std::hint::black_box(checksum);
    let elapsed = start.elapsed().as_secs_f64();
    (tasks * per_task) as f64 / elapsed
}

fn best(samples: &[f64]) -> f64 {
    samples.iter().copied().fold(f64::MIN, f64::max)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const SQL: &str = "SELECT id, email, age FROM users";
    const POOL: usize = 8;

    let rus = Arc::new(RusqlitePool::new(POOL));
    // Same connection count as the rusqlite arm, and no pre-acquire ping, so
    // neither arm is starved and neither pays for a knob the other does not.
    let native = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(POOL as u32)
        .test_before_acquire(false)
        .connect_with(
            db_path()
                .parse::<sqlx::sqlite::SqliteConnectOptions>()?
                .read_only(true),
        )
        .await?;

    println!("\nfind_many_1000, read-only, {POOL} connections per arm");
    println!("throughput = total queries/sec across all tasks (higher is better)\n");
    println!(
        "{:>6}  {:>16}  {:>16}  {:>10}",
        "tasks", "rusqlite q/s", "sqlx-sqlite q/s", "speedup"
    );
    println!("{}", "-".repeat(56));

    for tasks in [1usize, 2, 4, 8, 16] {
        let per_task = (400 / tasks).max(25);

        let mut r_samples = Vec::new();
        let mut s_samples = Vec::new();
        for _ in 0..REPEATS {
            let rus2 = Arc::clone(&rus);
            r_samples.push(
                throughput(tasks, per_task, move || {
                    let rus = Arc::clone(&rus2);
                    async move { rus.fetch(SQL).await }
                })
                .await,
            );

            let pool = native.clone();
            s_samples.push(
                throughput(tasks, per_task, move || {
                    let pool = pool.clone();
                    async move {
                        sqlx::query_as::<sqlx::Sqlite, User>(SQL)
                            .fetch_all(&pool)
                            .await
                            .unwrap()
                            .len()
                    }
                })
                .await,
            );
        }

        let (r, s) = (best(&r_samples), best(&s_samples));
        println!("{tasks:>6}  {r:>16.0}  {s:>16.0}  {:>9.2}x", r / s);
    }

    println!(
        "\n(tokio max_blocking_threads defaults to 512; at 16 tasks the rusqlite\n \
         arm is using 8 of them at a time, bounded by the connection count.)"
    );

    Ok(())
}
