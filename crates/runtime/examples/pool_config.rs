//! What ruprizzle's `PoolConfig` defaults cost per query.
//!
//! `PoolConfig::default()` sets `test_before_acquire: true`, so `sqlx` calls
//! `Connection::ping()` on the idle connection before every checkout
//! (`sqlx-core/src/pool/inner.rs`, `check_idle_conn`).
//!
//! On SQLite that ping is a round-trip to the connection's worker thread. On
//! **Postgres** it is `write_sync()` + `wait_until_ready()`
//! (`sqlx-postgres/src/connection/mod.rs`) — a full network round-trip to the
//! server, added to every query. This harness can only measure the SQLite side;
//! the Postgres cost is one RTT by construction and is called out in the plan
//! as requiring a live-database check.
//!
//! Repeated, because the effect sits close to this machine's scheduling noise
//! on a single run.

#![allow(dead_code)]

use std::time::Instant;

use sqlx::any::AnyPoolOptions;
use sqlx::FromRow;

const REPEATS: usize = 7;

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

/// Interleaves the two variants within each repeat, so thermal drift and
/// scheduler noise hit both arms equally instead of favouring whichever ran
/// first.
async fn paired(
    iters: u32,
    a: &sqlx::Pool<sqlx::Any>,
    b: &sqlx::Pool<sqlx::Any>,
    sql: &str,
) -> (Vec<f64>, Vec<f64>) {
    let run = |pool: &sqlx::Pool<sqlx::Any>, sql: &str| {
        let pool = pool.clone();
        let sql = sql.to_owned();
        async move {
            sqlx::query_as::<sqlx::Any, User>(&sql)
                .bind(500i64)
                .fetch_all(&pool)
                .await
                .unwrap()
                .len()
        }
    };

    let mut checksum = 0usize;
    for _ in 0..3 {
        checksum += run(a, sql).await + run(b, sql).await;
    }

    let (mut sa, mut sb) = (Vec::new(), Vec::new());
    for _ in 0..REPEATS {
        let start = Instant::now();
        for _ in 0..iters {
            checksum += run(a, sql).await;
        }
        sa.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(iters));

        let start = Instant::now();
        for _ in 0..iters {
            checksum += run(b, sql).await;
        }
        sb.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(iters));
    }
    std::hint::black_box(checksum);
    (sa, sb)
}

fn summarise(label: &str, mut on: Vec<f64>, mut off: Vec<f64>) {
    on.sort_by(f64::total_cmp);
    off.sort_by(f64::total_cmp);
    let (mon, moff) = (on[on.len() / 2], off[off.len() / 2]);
    println!("\n{label}");
    println!(
        "  test_before_acquire=true   min {:>8.2}  median {mon:>8.2}  max {:>8.2} us",
        on[0],
        on[on.len() - 1]
    );
    println!(
        "  test_before_acquire=false  min {:>8.2}  median {moff:>8.2}  max {:>8.2} us",
        off[0],
        off[off.len() - 1]
    );
    println!(
        "  -> ping cost: {:+.2} us/query ({:+.1}%)   [ranges {}overlap]",
        mon - moff,
        (mon / moff - 1.0) * 100.0,
        if on[0] > off[off.len() - 1] { "do not " } else { "" }
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = format!("sqlite:///{}?mode=ro", db_path());

    let on = AnyPoolOptions::new()
        .max_connections(10)
        .test_before_acquire(true)
        .connect(&url)
        .await?;
    let off = AnyPoolOptions::new()
        .max_connections(10)
        .test_before_acquire(false)
        .connect(&url)
        .await?;

    let (a, b) = paired(
        3000,
        &on,
        &off,
        "SELECT id, email, age FROM users WHERE id = ?",
    )
    .await;
    summarise("select_by_pk (1 row)", a, b);

    let (a, b) = paired(200, &on, &off, "SELECT id, email, age FROM users").await;
    summarise("find_many_1000 (1000 rows)", a, b);

    println!(
        "\nOn Postgres the same checkout costs one network round-trip \
         (write_sync + wait_until_ready),\nwhich on a hosted database is \
         milliseconds, not microseconds."
    );

    Ok(())
}
