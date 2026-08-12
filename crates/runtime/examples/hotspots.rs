//! Quantifies the specific ORM-layer hotspots the enhancement plan proposes
//! to fix, so each one gets a number rather than an assertion.

#![allow(dead_code)]

use std::time::Instant;

use ruprizzle::{Column, Model, SelectQuery};
use sqlx::FromRow;

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

#[derive(Debug, Clone, Default, FromRow)]
struct User {
    id: i64,
    email: String,
    age: i64,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(User);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for User {
    fn from_rusqlite_row(row: &mut ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.take::<i64>(0)?,
            email: row.take::<String>(1)?,
            age: row.take::<i64>(2)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_AGE: Column<User, i64> = Column::new("users", "age");

fn us(start: Instant, iters: u32) -> f64 {
    start.elapsed().as_secs_f64() * 1e6 / f64::from(iters)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = format!("sqlite:///{}?mode=ro", db_path());
    let any = ruprizzle::connect(&url).await?;

    // ---- 1. fetch_optional has no LIMIT 1 ----
    println!("\n--- 1. fetch_one / fetch_optional emit no LIMIT 1 ---");
    let q = SelectQuery::<User>::new(&any).filter(USER_AGE.gt(0i64));
    println!("  SQL for .filter(age > 0).fetch_optional():");
    println!("    {}", q.to_sql().sql);

    let iters = 100;
    let start = Instant::now();
    for _ in 0..iters {
        let u = SelectQuery::<User>::new(&any)
            .filter(USER_AGE.gt(0i64))
            .fetch_optional()
            .await
            .unwrap();
        std::hint::black_box(u);
    }
    let without = us(start, iters);
    let start = Instant::now();
    for _ in 0..iters {
        let u = SelectQuery::<User>::new(&any)
            .filter(USER_AGE.gt(0i64))
            .limit(1)
            .fetch_optional()
            .await
            .unwrap();
        std::hint::black_box(u);
    }
    let with = us(start, iters);
    println!(
        "  fetch_optional() as-is        {without:>9.1} us  (fetches + decodes all 1000 rows)"
    );
    println!("  fetch_optional() with .limit(1){with:>9.1} us");
    println!(
        "  -> cost of the missing LIMIT   {:>8.1} us  ({:.0}x)",
        without - with,
        without / with
    );

    // ---- 2. count() keeps ORDER BY ----
    println!("\n--- 2. count() rewrites SQL by string surgery ---");
    let q = SelectQuery::<User>::new(&any)
        .filter(USER_AGE.gt(0i64))
        .order_by(USER_AGE.desc())
        .limit(10);
    println!("  base SQL: {}", q.to_sql().sql);
    let r = SelectQuery::<User>::new(&any)
        .filter(USER_AGE.gt(0i64))
        .order_by(USER_AGE.desc())
        .limit(10)
        .count()
        .await;
    match r {
        Ok(n) => println!("  count() -> Ok({n})   <- LIMIT 10 is still in the counted SQL"),
        Err(e) => println!("  count() -> Err({e})"),
    }

    // ---- 3. per-query allocations in the compile path ----
    println!("\n--- 3. compile-path allocations (no I/O) ---");
    let iters = 100_000;
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(ruprizzle::compile::dialect_for_pool(&any));
    }
    println!(
        "  Executor::dialect() -> Box<dyn DbDialect>  {:>7.3} us/call",
        us(start, iters)
    );

    let q = SelectQuery::<User>::new(&any).filter(USER_ID.eq(1i64));
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(q.to_sql());
    }
    let total = us(start, iters);
    println!("  full to_sql() for select-by-pk             {total:>7.3} us/call");

    // ---- 4. SELECT * vs explicit projection ----
    println!("\n--- 4. default projection ---");
    println!("  {}", SelectQuery::<User>::new(&any).to_sql().sql);
    println!("  -> `SELECT *`: column set is whatever the table has, not what the model needs,");
    println!("     and the ordinals are unknown, which forces name-based decoding.");

    // ---- 5. name vs ordinal decode, isolated ----
    println!("\n--- 5. decode by name vs by ordinal (1000 rows x 3 cols) ---");
    let rows = sqlx::query("SELECT id, email, age FROM users")
        .fetch_all(&any)
        .await?;
    use sqlx::Row;
    let iters = 500;
    let start = Instant::now();
    for _ in 0..iters {
        for r in &rows {
            std::hint::black_box(User {
                id: ruprizzle::decode::direct(r, "id").unwrap(),
                email: ruprizzle::decode::direct(r, "email").unwrap(),
                age: ruprizzle::decode::direct(r, "age").unwrap(),
            });
        }
    }
    let by_name = us(start, iters);
    let start = Instant::now();
    for _ in 0..iters {
        for r in &rows {
            std::hint::black_box(User {
                id: r.get(0),
                email: r.get(1),
                age: r.get(2),
            });
        }
    }
    let by_ord = us(start, iters);
    println!("  by name    {by_name:>9.1} us");
    println!("  by ordinal {by_ord:>9.1} us");
    println!(
        "  -> {:.0}% of decode time is the name->ordinal hash lookup",
        (1.0 - by_ord / by_name) * 100.0
    );

    // ---- 6. the boolean decode helper's error-path fallback ----
    println!("\n--- 6. decode::boolean tries i64 first, then falls back ---");
    let iters = 200;
    let start = Instant::now();
    for _ in 0..iters {
        for r in &rows {
            // `id` is INTEGER: the i64 attempt succeeds, no error is built.
            std::hint::black_box(ruprizzle::decode::boolean(r, "id").unwrap());
        }
    }
    let happy = us(start, iters);
    let start = Instant::now();
    for _ in 0..iters {
        for r in &rows {
            // `email` is TEXT: the i64 attempt fails, so sqlx builds and boxes
            // an Error that is then thrown away. This is the Postgres BOOLEAN path.
            std::hint::black_box(ruprizzle::decode::boolean(r, "email").is_ok());
        }
    }
    let sad = us(start, iters);
    println!("  1000x boolean() on an INTEGER column (hit)   {happy:>9.1} us");
    println!("  1000x boolean() on a TEXT column (miss+box)  {sad:>9.1} us");
    println!(
        "  -> discarded-error path costs {:.0}x more per column",
        sad / happy
    );

    Ok(())
}
