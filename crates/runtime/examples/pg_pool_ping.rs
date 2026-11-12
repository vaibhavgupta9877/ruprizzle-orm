//! Live Postgres measurement of the `test_before_acquire` ping (Enhancements2
//! §3.3, task P0-1).
//!
//! Same paired-interleaving method as `pool_config.rs`, but against a real
//! server through the native `sqlx` Postgres pool. Each arm is a separate pool
//! with one connection, so the only difference is the ping before checkout.
//!
//! Run:
//! ```text
//! set RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test
//! cargo run --release -p ruprizzle --example pg_pool_ping
//! ```

use std::time::{Duration, Instant};

use sqlx::postgres::PgPoolOptions;

const ITERS: usize = 2000;
const REPEATS: usize = 7;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

async fn time_arm(pool: &sqlx::PgPool, iters: usize) -> f64 {
    // One warm-up query per arm so the connection exists before timing.
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .expect("warm-up");
    let start = Instant::now();
    for _ in 0..iters {
        sqlx::query("SELECT 1").execute(pool).await.expect("query");
    }
    start.elapsed().as_secs_f64() * 1e6 / iters as f64
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("RUPRIZZLE_TEST_PG_URL")?;
    let url = url.split("?schema=").next().unwrap().to_string();

    let connect = |ping: bool| {
        PgPoolOptions::new()
            .max_connections(1)
            .test_before_acquire(ping)
            .acquire_timeout(Duration::from_secs(20))
            .connect(&url)
    };
    let on = connect(true).await?;
    let off = connect(false).await?;

    let (mut a, mut b) = (Vec::new(), Vec::new());
    for r in 0..REPEATS {
        // Alternate which arm runs first so drift hits both equally.
        if r % 2 == 0 {
            a.push(time_arm(&on, ITERS).await);
            b.push(time_arm(&off, ITERS).await);
        } else {
            b.push(time_arm(&off, ITERS).await);
            a.push(time_arm(&on, ITERS).await);
        }
    }

    let (min_a, max_a) = (
        a.iter().cloned().fold(f64::INFINITY, f64::min),
        a.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let (min_b, max_b) = (
        b.iter().cloned().fold(f64::INFINITY, f64::min),
        b.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let (ma, mb) = (median(a), median(b));

    println!(
        "SELECT 1 over a live Postgres connection, {ITERS} queries x {REPEATS} interleaved repeats, us/query"
    );
    println!("  test_before_acquire=true   min {min_a:>9.1}  median {ma:>9.1}  max {max_a:>9.1}");
    println!("  test_before_acquire=false  min {min_b:>9.1}  median {mb:>9.1}  max {max_b:>9.1}");
    println!(
        "  ping cost: {:+.1} us/query ({:+.1}%)",
        ma - mb,
        (ma - mb) / mb * 100.0
    );
    Ok(())
}
