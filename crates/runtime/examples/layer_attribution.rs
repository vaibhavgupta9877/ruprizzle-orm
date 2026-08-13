//! Layer-attribution experiment: how much of ruprizzle's cost is the
//! `sqlx::Any` driver and how much is the ORM layer on top of it?
//!
//! Runs the same three query shapes at four layers against the same SQLite
//! file used by the cross-ORM benchmark:
//!
//! 1. `sqlite-native`  — `sqlx::Sqlite`, `query_as` into a struct
//! 2. `any-driver`     — `sqlx::Any`, `query_as` into a struct
//! 3. `any-manual`     — `sqlx::Any` + ruprizzle's `decode::*` helpers
//! 4. `ruprizzle`      — the full builder path
//!
//! (2) minus (1) is the `Any` tax. (4) minus (3) is the builder tax.

#![allow(dead_code)]

use std::time::Instant;

use ruprizzle::{Column, Model, SelectQuery};
use sqlx::{FromRow, Row};

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
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            email: ::ruprizzle::rusqlite::get::<String>(row, 1)?,
            age: ::ruprizzle::rusqlite::get::<i64>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for User {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            email: row.get::<String>(1)?,
            age: row.get::<i64>(2)?,
        })
    }
}

impl Model for User {
    const TABLE: &'static str = "users";
}

const USER_ID: Column<User, i64> = Column::new("users", "id");
const USER_AGE: Column<User, i64> = Column::new("users", "age");

/// The manual-decode shape, mirroring what codegen emits for `User`.
fn decode_user(row: &sqlx::any::AnyRow) -> Result<User, sqlx::Error> {
    Ok(User {
        id: ruprizzle::decode::direct(row, "id")?,
        email: ruprizzle::decode::direct(row, "email")?,
        age: ruprizzle::decode::direct(row, "age")?,
    })
}

async fn bench<F, Fut>(label: &str, iters: u32, mut f: F) -> f64
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
    println!("{label:<40} {us:>10.1} us/op   (checksum {checksum})");
    us
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = format!("sqlite:///{}?mode=ro", db_path());

    // Single connection everywhere so pool checkout is not part of the delta.
    let native = sqlx::SqlitePool::connect_with(
        url.replace("sqlite:///", "")
            .parse::<sqlx::sqlite::SqliteConnectOptions>()?
            .read_only(true),
    )
    .await?;
    let any = ruprizzle::connect(&url).await?;

    println!("\n=== select_by_pk (1 row) ===");
    let a = bench("1 sqlite-native  query_as", 2000, || async {
        let u: User = sqlx::query_as("SELECT id, email, age FROM users WHERE id = ?")
            .bind(500i64)
            .fetch_one(&native)
            .await
            .unwrap();
        u.id as usize
    })
    .await;
    let b = bench("2 any-driver     query_as", 2000, || async {
        let u: User =
            sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users WHERE id = ?")
                .bind(500i64)
                .fetch_one(&any)
                .await
                .unwrap();
        u.id as usize
    })
    .await;
    let c = bench("3 any-manual     decode helpers", 2000, || async {
        let row = sqlx::query("SELECT id, email, age FROM users WHERE id = ?")
            .bind(500i64)
            .fetch_one(&any)
            .await
            .unwrap();
        decode_user(&row).unwrap().id as usize
    })
    .await;
    let d = bench("4 ruprizzle      SelectQuery", 2000, || async {
        let u = SelectQuery::<User>::new(&any)
            .filter(USER_ID.eq(500i64))
            .fetch_optional()
            .await
            .unwrap();
        u.unwrap().id as usize
    })
    .await;
    report(a, b, c, d);

    println!("\n=== find_many_1000 ===");
    let a = bench("1 sqlite-native  query_as", 200, || async {
        let v: Vec<User> = sqlx::query_as("SELECT id, email, age FROM users")
            .fetch_all(&native)
            .await
            .unwrap();
        v.len()
    })
    .await;
    let b = bench("2 any-driver     query_as", 200, || async {
        let v: Vec<User> = sqlx::query_as::<sqlx::Any, User>("SELECT id, email, age FROM users")
            .fetch_all(&any)
            .await
            .unwrap();
        v.len()
    })
    .await;
    let c = bench("3 any-manual     decode helpers", 200, || async {
        let rows = sqlx::query("SELECT id, email, age FROM users")
            .fetch_all(&any)
            .await
            .unwrap();
        rows.iter().map(|r| decode_user(r).unwrap()).count()
    })
    .await;
    let d = bench("4 ruprizzle      SelectQuery", 200, || async {
        SelectQuery::<User>::new(&any)
            .fetch_all()
            .await
            .unwrap()
            .len()
    })
    .await;
    report(a, b, c, d);

    println!("\n=== find_filtered_ordered (age > 0 ORDER BY age DESC) ===");
    let a = bench("1 sqlite-native  query_as", 200, || async {
        let v: Vec<User> =
            sqlx::query_as("SELECT id, email, age FROM users WHERE age > ? ORDER BY age DESC")
                .bind(0i64)
                .fetch_all(&native)
                .await
                .unwrap();
        v.len()
    })
    .await;
    let b = bench("2 any-driver     query_as", 200, || async {
        let v: Vec<User> = sqlx::query_as::<sqlx::Any, User>(
            "SELECT id, email, age FROM users WHERE age > ? ORDER BY age DESC",
        )
        .bind(0i64)
        .fetch_all(&any)
        .await
        .unwrap();
        v.len()
    })
    .await;
    let c = bench("3 any-manual     decode helpers", 200, || async {
        let rows = sqlx::query("SELECT id, email, age FROM users WHERE age > ? ORDER BY age DESC")
            .bind(0i64)
            .fetch_all(&any)
            .await
            .unwrap();
        rows.iter().map(|r| decode_user(r).unwrap()).count()
    })
    .await;
    let d = bench("4 ruprizzle      SelectQuery", 200, || async {
        SelectQuery::<User>::new(&any)
            .filter(USER_AGE.gt(0i64))
            .order_by(USER_AGE.desc())
            .fetch_all()
            .await
            .unwrap()
            .len()
    })
    .await;
    report(a, b, c, d);

    println!("\n=== isolating name-lookup vs ordinal decode (1000 rows, no I/O per col) ===");
    let rows = sqlx::query("SELECT id, email, age FROM users")
        .fetch_all(&any)
        .await?;
    let start = Instant::now();
    let mut n = 0usize;
    for _ in 0..200 {
        n += rows.iter().map(|r| decode_user(r).unwrap()).count();
    }
    let by_name = start.elapsed().as_secs_f64() * 1e6 / 200.0;
    let start = Instant::now();
    let mut m = 0usize;
    for _ in 0..200 {
        m += rows
            .iter()
            .map(|r| User {
                id: r.get::<i64, _>(0),
                email: r.get::<String, _>(1),
                age: r.get::<i64, _>(2),
            })
            .count();
    }
    let by_ordinal = start.elapsed().as_secs_f64() * 1e6 / 200.0;
    println!("decode 1000 rows by column NAME          {by_name:>10.1} us  ({n})");
    println!("decode 1000 rows by column ORDINAL       {by_ordinal:>10.1} us  ({m})");
    println!(
        "-> name lookup overhead                  {:>10.1} us  ({:.0}%)",
        by_name - by_ordinal,
        (by_name / by_ordinal - 1.0) * 100.0
    );

    Ok(())
}

fn report(native: f64, any: f64, any_manual: f64, ruprizzle: f64) {
    println!(
        "  Any-driver tax   (2-1): {:>8.1} us  ({:.2}x native)",
        any - native,
        any / native
    );
    println!("  decode-helper tax(3-2): {:>8.1} us", any_manual - any);
    println!(
        "  builder tax      (4-3): {:>8.1} us",
        ruprizzle - any_manual
    );
    println!(
        "  total vs native  (4-1): {:>8.1} us  ({:.2}x native)",
        ruprizzle - native,
        ruprizzle / native
    );
}
