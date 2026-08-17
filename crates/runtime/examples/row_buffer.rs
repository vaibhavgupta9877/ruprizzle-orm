//! sqlx-sqlite ships every row to the async task through a bounded `flume`
//! channel whose default depth is 50 rows. That knob lives on
//! `SqliteConnectOptions::row_buffer_size` and is unreachable through
//! `sqlx::Any`, which is the pool ruprizzle builds.
//!
//! This measures what the knob is worth, i.e. what the `Any` abstraction costs
//! us by hiding it.

use std::time::Instant;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "\n{:<16} {:>14} {:>16}",
        "row_buffer_size", "users (1k)", "posts (10k)"
    );
    println!("{}", "-".repeat(50));

    let mut baseline = (0.0f64, 0.0f64);
    for size in [50usize, 200, 1000, 4096, 16384] {
        let opts: sqlx::sqlite::SqliteConnectOptions = db_path().parse()?;
        let pool =
            sqlx::SqlitePool::connect_with(opts.read_only(true).row_buffer_size(size)).await?;

        let t_users = time(&pool, "SELECT id, email, age FROM users", 200).await;
        let t_posts = time(&pool, "SELECT id, author_id, title FROM posts", 40).await;
        if size == 50 {
            baseline = (t_users, t_posts);
        }
        println!(
            "{size:<16} {t_users:>10.1} us {:>4} {t_posts:>11.1} us {:>4}",
            pct(baseline.0, t_users),
            pct(baseline.1, t_posts)
        );
        pool.close().await;
    }

    println!("\n(`ruprizzle::connect` builds an `AnyPool`; the Any driver reads only the URL,");
    println!(" so this option cannot be set through the public API today.)");
    Ok(())
}

fn pct(base: f64, v: f64) -> String {
    if base == 0.0 {
        return String::new();
    }
    format!("{:+.0}%", (v / base - 1.0) * 100.0)
}

async fn time(pool: &sqlx::SqlitePool, sql: &str, iters: u32) -> f64 {
    for _ in 0..3 {
        let _ = sqlx::query(sql).fetch_all(pool).await.unwrap();
    }
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(sqlx::query(sql).fetch_all(pool).await.unwrap());
    }
    start.elapsed().as_secs_f64() * 1e6 / f64::from(iters)
}
