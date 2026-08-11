//! Where does the 1.6 us/row go? Isolates sqlx's own floor on SQLite.
//!
//! If `execute` (rows produced by SQLite but discarded by sqlx) is far cheaper
//! than `fetch_all` (rows materialised into owned `SqliteRow`s), then the cost
//! is sqlx's per-row materialisation and channel transport, not SQLite, not
//! the `Any` wrapper, and not ruprizzle.

use std::time::Instant;

use sqlx::{Executor, Row};

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

async fn bench<F, Fut>(label: &str, iters: u32, rows: f64, mut f: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = usize>,
{
    let mut checksum = 0usize;
    for _ in 0..3.min(iters) {
        checksum += f().await;
    }
    let start = Instant::now();
    for _ in 0..iters {
        checksum += f().await;
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(iters);
    if rows > 0.0 {
        println!("{label:<44} {us:>9.1} us/op  {:>7.3} us/row  ({checksum})", us / rows);
    } else {
        println!("{label:<44} {us:>9.1} us/op                  ({checksum})");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: sqlx::sqlite::SqliteConnectOptions = db_path().parse()?;
    let pool = sqlx::SqlitePool::connect_with(opts.clone().read_only(true)).await?;

    println!("\n--- SQLite work vs sqlx transport (users: 1000 rows x 3 cols) ---");
    bench("execute      SELECT * (rows discarded)", 300, 0.0, || async {
        pool.execute("SELECT id, email, age FROM users").await.unwrap().rows_affected() as usize
    })
    .await;
    bench("fetch_all    SELECT * (rows kept)", 300, 1000.0, || async {
        sqlx::query("SELECT id, email, age FROM users").fetch_all(&pool).await.unwrap().len()
    })
    .await;
    bench("fetch_all    SELECT id only (1 col)", 300, 1000.0, || async {
        sqlx::query("SELECT id FROM users").fetch_all(&pool).await.unwrap().len()
    })
    .await;
    bench("fetch_all    SELECT email only (1 text col)", 300, 1000.0, || async {
        sqlx::query("SELECT email FROM users").fetch_all(&pool).await.unwrap().len()
    })
    .await;
    bench("scalar       SELECT count(*)", 300, 0.0, || async {
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&pool).await.unwrap();
        n as usize
    })
    .await;
    bench("fetch_all    LIMIT 1", 300, 0.0, || async {
        sqlx::query("SELECT id, email, age FROM users LIMIT 1").fetch_all(&pool).await.unwrap().len()
    })
    .await;

    println!("\n--- marginal cost per row (posts: 10 000 rows x 3 cols) ---");
    bench("execute      posts (rows discarded)", 60, 0.0, || async {
        pool.execute("SELECT id, author_id, title FROM posts").await.unwrap().rows_affected() as usize
    })
    .await;
    bench("fetch_all    posts (rows kept)", 60, 10000.0, || async {
        sqlx::query("SELECT id, author_id, title FROM posts").fetch_all(&pool).await.unwrap().len()
    })
    .await;

    println!("\n--- fetch_all vs streaming fold (no Vec<Row> materialisation) ---");
    use futures_util::TryStreamExt;
    bench("stream+fold  posts (decode, drop each row)", 60, 10000.0, || async {
        let mut s = sqlx::query("SELECT id, author_id, title FROM posts").fetch(&pool);
        let mut n = 0usize;
        while let Some(r) = s.try_next().await.unwrap() {
            let _: i64 = r.get(0);
            n += 1;
        }
        n
    })
    .await;

    println!("\n--- round-trip floor: how expensive is one trivial query? ---");
    bench("SELECT 1", 3000, 0.0, || async {
        let n: i64 = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await.unwrap();
        n as usize
    })
    .await;
    bench("SELECT 1  (Any driver)", 3000, 0.0, || async { 0 }).await;

    Ok(())
}
