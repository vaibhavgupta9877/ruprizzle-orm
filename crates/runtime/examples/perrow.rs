//! Per-row throughput at every layer, with repeats.
//!
//! `bottlenecks` showed that essentially all of ruprizzle's remaining cost is
//! per-row: `include_posts` is 16.4 ms of which 14.8 ms is just pulling 10 000
//! rows through `sqlx::Any`. This measures how far that per-row cost can fall,
//! and by which route, so the enhancement plan can be costed against a real
//! floor instead of against better-sqlite3 (a different language and a
//! different concurrency model).
//!
//! Layers, cheapest first:
//!
//! 1. `rusqlite`         — synchronous, in-process, the Rust analogue of
//!                         better-sqlite3. This is the floor.
//! 2. `sqlx native`      — `SqlitePool`, default `row_buffer_size` (50)
//! 3. `sqlx native +buf` — `SqlitePool`, `row_buffer_size(16384)`
//! 4. `sqlx Any`         — `AnyPool`, what ruprizzle builds today
//! 5. `ruprizzle`        — the full builder path
//!
//! Every layer materialises the same owned `User` / `Post` structs, so the
//! comparison is like-for-like: no layer is credited for skipping work.

#![allow(dead_code)]

use std::time::Instant;

use ruprizzle::{Model, SelectQuery};
use sqlx::FromRow;

/// How many timed repeats per layer. Reported as min/median/max, because
/// run-to-run spread on this workload is wide enough that a single number
/// invites false conclusions.
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
    const COLUMNS: &'static [&'static str] = &["id", "email", "age"];
}

#[derive(Debug, Clone, Default, FromRow)]
struct Post {
    id: i64,
    author_id: i64,
    title: String,
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Post);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Post {
    fn from_rusqlite_row(row: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: ::ruprizzle::rusqlite::get::<i64>(row, 0)?,
            author_id: ::ruprizzle::rusqlite::get::<i64>(row, 1)?,
            title: ::ruprizzle::rusqlite::get::<String>(row, 2)?,
        })
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Post {
    fn from_owned_row(row: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Self {
            id: row.get::<i64>(0)?,
            author_id: row.get::<i64>(1)?,
            title: row.get::<String>(2)?,
        })
    }
}

impl Model for Post {
    const TABLE: &'static str = "posts";
    const COLUMNS: &'static [&'static str] = &["id", "author_id", "title"];
}

/// One measured layer: the spread over `REPEATS` timed runs.
struct Row {
    label: &'static str,
    samples: Vec<f64>,
    rows: f64,
}

impl Row {
    fn stats(&self) -> (f64, f64, f64) {
        let mut s = self.samples.clone();
        s.sort_by(f64::total_cmp);
        (s[0], s[s.len() / 2], s[s.len() - 1])
    }
}

fn report(title: &str, rows: &[Row]) {
    println!("\n=== {title} ===");
    println!(
        "{:<28} {:>10} {:>10} {:>10}   {:>9}  {:>7}",
        "layer", "min us", "median", "max", "us/row", "vs floor"
    );
    println!("{}", "-".repeat(82));
    let floor = rows[0].stats().1;
    for r in rows {
        let (lo, med, hi) = r.stats();
        println!(
            "{:<28} {lo:>10.1} {med:>10.1} {hi:>10.1}   {:>9.3}  {:>6.2}x",
            r.label,
            med / r.rows,
            med / floor
        );
    }
}

/// Times `f` `REPEATS` times, `iters` operations per repeat.
async fn sample<F, Fut>(iters: u32, mut f: F) -> Vec<f64>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = usize>,
{
    let mut out = Vec::with_capacity(REPEATS);
    let mut checksum = 0usize;
    for _ in 0..3 {
        checksum += f().await;
    }
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

/// The synchronous floor. Not async, so it is timed with a plain loop.
fn sample_rusqlite(sql: &str, iters: u32) -> Vec<f64> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    let mut stmt = conn.prepare(sql).unwrap();
    let mut run = || {
        let rows = stmt
            .query_map([], |r| {
                Ok(User {
                    id: r.get(0)?,
                    email: r.get(1)?,
                    age: r.get(2)?,
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows.len()
    };
    let mut checksum = 0usize;
    for _ in 0..3 {
        checksum += run();
    }
    let mut out = Vec::with_capacity(REPEATS);
    for _ in 0..REPEATS {
        let start = Instant::now();
        for _ in 0..iters {
            checksum += run();
        }
        out.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(iters));
    }
    std::hint::black_box(checksum);
    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = format!("sqlite:///{}?mode=ro", db_path());
    let opts: sqlx::sqlite::SqliteConnectOptions = db_path().parse()?;

    let native = sqlx::SqlitePool::connect_with(opts.clone().read_only(true)).await?;
    let buffered =
        sqlx::SqlitePool::connect_with(opts.clone().read_only(true).row_buffer_size(16384)).await?;
    let any = ruprizzle::connect(&url).await?;

    // ---------------- users: 1000 rows, 3 columns -------------------
    const USERS_SQL: &str = "SELECT id, email, age FROM users";
    let mut rows = Vec::new();

    rows.push(Row {
        label: "1 rusqlite (sync)",
        samples: sample_rusqlite(USERS_SQL, 300),
        rows: 1000.0,
    });
    rows.push(Row {
        label: "2 sqlx native",
        samples: sample(300, || async {
            sqlx::query_as::<sqlx::Sqlite, User>(USERS_SQL)
                .fetch_all(&native)
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 1000.0,
    });
    rows.push(Row {
        label: "3 sqlx native +row_buffer",
        samples: sample(300, || async {
            sqlx::query_as::<sqlx::Sqlite, User>(USERS_SQL)
                .fetch_all(&buffered)
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 1000.0,
    });
    rows.push(Row {
        label: "4 sqlx Any",
        samples: sample(300, || async {
            sqlx::query_as::<sqlx::Any, User>(USERS_SQL)
                .fetch_all(&any)
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 1000.0,
    });
    rows.push(Row {
        label: "5 ruprizzle SelectQuery",
        samples: sample(300, || async {
            SelectQuery::<User>::new(&any)
                .fetch_all()
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 1000.0,
    });
    report("users: 1000 rows x (i64, String, i64)", &rows);

    // ---------------- posts: 10 000 rows, 3 columns -----------------
    const POSTS_SQL: &str = "SELECT id, author_id, title FROM posts";
    let mut rows = Vec::new();

    {
        // Same shape as `sample_rusqlite` but decoding `Post`.
        let conn = rusqlite::Connection::open_with_flags(
            db_path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut stmt = conn.prepare(POSTS_SQL)?;
        let mut run = || {
            stmt.query_map([], |r| {
                Ok(Post {
                    id: r.get(0)?,
                    author_id: r.get(1)?,
                    title: r.get(2)?,
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len()
        };
        let mut checksum = 0usize;
        for _ in 0..3 {
            checksum += run();
        }
        let mut samples = Vec::with_capacity(REPEATS);
        for _ in 0..REPEATS {
            let start = Instant::now();
            for _ in 0..40 {
                checksum += run();
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 40.0);
        }
        std::hint::black_box(checksum);
        rows.push(Row {
            label: "1 rusqlite (sync)",
            samples,
            rows: 10000.0,
        });
    }
    rows.push(Row {
        label: "2 sqlx native",
        samples: sample(40, || async {
            sqlx::query_as::<sqlx::Sqlite, Post>(POSTS_SQL)
                .fetch_all(&native)
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 10000.0,
    });
    rows.push(Row {
        label: "3 sqlx native +row_buffer",
        samples: sample(40, || async {
            sqlx::query_as::<sqlx::Sqlite, Post>(POSTS_SQL)
                .fetch_all(&buffered)
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 10000.0,
    });
    rows.push(Row {
        label: "4 sqlx Any",
        samples: sample(40, || async {
            sqlx::query_as::<sqlx::Any, Post>(POSTS_SQL)
                .fetch_all(&any)
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 10000.0,
    });
    rows.push(Row {
        label: "5 ruprizzle SelectQuery",
        samples: sample(40, || async {
            SelectQuery::<Post>::new(&any)
                .fetch_all()
                .await
                .unwrap()
                .len()
        })
        .await,
        rows: 10000.0,
    });
    report("posts: 10 000 rows x (i64, i64, String)", &rows);

    // ---------------- point query: 1 row ----------------------------
    const PK_SQL: &str = "SELECT id, email, age FROM users WHERE id = ?";
    let mut rows = Vec::new();
    {
        let conn = rusqlite::Connection::open_with_flags(
            db_path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut stmt = conn.prepare(PK_SQL)?;
        let mut run = || {
            stmt.query_row([500i64], |r| {
                Ok(User {
                    id: r.get(0)?,
                    email: r.get(1)?,
                    age: r.get(2)?,
                })
            })
            .unwrap()
            .id as usize
        };
        let mut checksum = 0usize;
        for _ in 0..3 {
            checksum += run();
        }
        let mut samples = Vec::with_capacity(REPEATS);
        for _ in 0..REPEATS {
            let start = Instant::now();
            for _ in 0..3000 {
                checksum += run();
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 3000.0);
        }
        std::hint::black_box(checksum);
        rows.push(Row {
            label: "1 rusqlite (sync)",
            samples,
            rows: 1.0,
        });
    }
    rows.push(Row {
        label: "2 sqlx native",
        samples: sample(3000, || async {
            sqlx::query_as::<sqlx::Sqlite, User>(PK_SQL)
                .bind(500i64)
                .fetch_one(&native)
                .await
                .unwrap()
                .id as usize
        })
        .await,
        rows: 1.0,
    });
    rows.push(Row {
        label: "4 sqlx Any",
        samples: sample(3000, || async {
            sqlx::query_as::<sqlx::Any, User>(PK_SQL)
                .bind(500i64)
                .fetch_one(&any)
                .await
                .unwrap()
                .id as usize
        })
        .await,
        rows: 1.0,
    });
    rows.push(Row {
        label: "5 ruprizzle SelectQuery",
        samples: sample(3000, || async {
            let c: ruprizzle::Column<User, i64> = ruprizzle::Column::new("users", "id");
            SelectQuery::<User>::new(&any)
                .filter(c.eq(500i64))
                .fetch_optional()
                .await
                .unwrap()
                .unwrap()
                .id as usize
        })
        .await,
        rows: 1.0,
    });
    report("select_by_pk: 1 row", &rows);

    Ok(())
}
