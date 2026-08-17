//! Is the `rusqlite` floor reachable from an async ORM?
//!
//! `perrow` shows `rusqlite` decoding rows 12-14x faster than `sqlx-sqlite`.
//! That number is only useful to ruprizzle if it survives the thing ruprizzle
//! would actually have to do: run the synchronous driver off the reactor.
//!
//! So this measures the same queries three ways:
//!
//! 1. `rusqlite` called inline (blocks the caller — not shippable, the floor)
//! 2. `rusqlite` inside `spawn_blocking` (one hop per *query*, not per row)
//! 3. `rusqlite` on a dedicated long-lived thread with a channel, which is
//!    what `sqlx-sqlite` already does — to check whether the thread model is
//!    the cost, or whether it is sqlx's per-row transport specifically.
//!
//! `sqlx native` is included as the incumbent.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use sqlx::FromRow;

const REPEATS: usize = 5;

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

fn map_users(stmt: &mut rusqlite::Statement<'_>) -> Vec<User> {
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
}

fn stats(mut s: Vec<f64>) -> (f64, f64) {
    s.sort_by(f64::total_cmp);
    (s[0], s[s.len() / 2])
}

fn line(label: &str, samples: Vec<f64>, rows: f64, floor: f64) -> f64 {
    let (lo, med) = stats(samples);
    println!(
        "{label:<44} {lo:>9.1} {med:>9.1}   {:>8.3} us/row  {:>6.2}x",
        med / rows,
        med / floor
    );
    med
}

async fn sample<F, Fut>(iters: u32, mut f: F) -> Vec<f64>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = usize>,
{
    let mut checksum = 0usize;
    for _ in 0..3 {
        checksum += f().await;
    }
    let mut out = Vec::with_capacity(REPEATS);
    for _ in 0..REPEATS {
        let start = Instant::now();
        for _ in 0..iters {
            checksum += f().await;
        }
        out.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(iters));
    }
    std::hint::black_box(checksum);
    out
}

/// A `rusqlite` connection pinned to its own thread, driven by a channel.
///
/// This is the shape `sqlx-sqlite` uses, minus the per-row channel send: the
/// whole result set crosses the boundary once.
struct ThreadConn {
    tx: std::sync::mpsc::Sender<(String, tokio::sync::oneshot::Sender<Vec<User>>)>,
}

impl ThreadConn {
    fn new() -> Self {
        let (tx, rx) =
            std::sync::mpsc::channel::<(String, tokio::sync::oneshot::Sender<Vec<User>>)>();
        std::thread::spawn(move || {
            let conn = open();
            let mut cache: std::collections::HashMap<String, rusqlite::Statement<'_>> =
                std::collections::HashMap::new();
            while let Ok((sql, reply)) = rx.recv() {
                let stmt = cache.entry(sql.clone()).or_insert_with(|| unsafe {
                    // The connection outlives every statement: it is owned
                    // by this thread and dropped only when the loop ends,
                    // after the cache. The transmute launders the borrow so
                    // the cache can be held alongside it.
                    std::mem::transmute::<rusqlite::Statement<'_>, rusqlite::Statement<'static>>(
                        conn.prepare(&sql).unwrap(),
                    )
                });
                let _ = reply.send(map_users(stmt));
            }
        });
        Self { tx }
    }

    async fn fetch(&self, sql: &str) -> Vec<User> {
        let (rtx, rrx) = tokio::sync::oneshot::channel();
        self.tx.send((sql.to_owned(), rtx)).unwrap();
        rrx.await.unwrap()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const SQL: &str = "SELECT id, email, age FROM users";
    const PK: &str = "SELECT id, email, age FROM users WHERE id = 500";

    for (title, sql, rows, iters) in [
        ("find_many_1000 (1000 rows)", SQL, 1000.0, 300u32),
        ("select_by_pk (1 row)", PK, 1.0, 3000u32),
    ] {
        println!("\n=== {title} ===");
        println!(
            "{:<44} {:>9} {:>9}   {:>16}  {:>6}",
            "layer", "min us", "median", "per row", "vs floor"
        );
        println!("{}", "-".repeat(92));

        // 1. inline rusqlite: the floor, blocks the caller.
        let floor = {
            let conn = open();
            let mut stmt = conn.prepare(sql)?;
            let mut checksum = 0usize;
            for _ in 0..3 {
                checksum += map_users(&mut stmt).len();
            }
            let mut samples = Vec::with_capacity(REPEATS);
            for _ in 0..REPEATS {
                let start = Instant::now();
                for _ in 0..iters {
                    checksum += map_users(&mut stmt).len();
                }
                samples.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(iters));
            }
            std::hint::black_box(checksum);
            let (lo, med) = stats(samples);
            println!(
                "{:<44} {lo:>9.1} {med:>9.1}   {:>8.3} us/row  {:>6.2}x",
                "1 rusqlite inline (blocks reactor)",
                med / rows,
                1.0
            );
            med
        };

        // 2. rusqlite inside spawn_blocking: one thread hop per query.
        //    The connection lives in a mutex so the closure can own it across
        //    hops; contention is nil with one caller.
        let shared = Arc::new(std::sync::Mutex::new(open()));
        line(
            "2 rusqlite via spawn_blocking",
            sample(iters, || {
                let shared = Arc::clone(&shared);
                async move {
                    tokio::task::spawn_blocking(move || {
                        let conn = shared.lock().unwrap();
                        let mut stmt = conn.prepare_cached(sql).unwrap();
                        map_users(&mut stmt).len()
                    })
                    .await
                    .unwrap()
                }
            })
            .await,
            rows,
            floor,
        );

        // 3. rusqlite on a dedicated thread, whole result set per hop.
        let threaded = ThreadConn::new();
        line(
            "3 rusqlite on dedicated thread + channel",
            sample(iters, || async { threaded.fetch(sql).await.len() }).await,
            rows,
            floor,
        );

        // 4. the incumbent.
        let native = sqlx::SqlitePool::connect_with(
            db_path()
                .parse::<sqlx::sqlite::SqliteConnectOptions>()?
                .read_only(true),
        )
        .await?;
        line(
            "4 sqlx-sqlite native (incumbent)",
            sample(iters, || async {
                sqlx::query_as::<sqlx::Sqlite, User>(sql)
                    .fetch_all(&native)
                    .await
                    .unwrap()
                    .len()
            })
            .await,
            rows,
            floor,
        );
    }

    Ok(())
}
