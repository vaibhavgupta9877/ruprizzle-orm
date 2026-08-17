//! Read-only probe: which Postgres types can `sqlx::Any` actually read?
//!
//! Runs `SELECT <expr>` for each type ruprizzle's Postgres dialect emits in
//! DDL. No DDL, no writes, no temp objects — just expressions.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sqlx::any::install_default_drivers();
    let url = std::env::var("RUPRIZZLE_TEST_PG_URL")?;
    let url = url.split("?schema=").next().unwrap().to_string();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(20))
        .connect(&url)
        .await?;

    // (ruprizzle scalar type, the DDL type its Postgres dialect emits, a literal)
    let cases = [
        ("String", "TEXT", "'x'::text"),
        ("Int", "INTEGER", "1::integer"),
        ("BigInt", "BIGINT", "1::bigint"),
        ("Float", "DOUBLE PRECISION", "1.0::double precision"),
        ("Boolean", "BOOLEAN", "true::boolean"),
        ("Bytes", "BYTEA", "'\\x00'::bytea"),
        ("Decimal", "NUMERIC", "1.5::numeric"),
        ("DateTime", "TIMESTAMPTZ", "now()::timestamptz"),
        ("Date", "DATE", "now()::date"),
        ("Time", "TIME", "now()::time"),
        (
            "Uuid",
            "UUID",
            "'00000000-0000-0000-0000-000000000000'::uuid",
        ),
        ("Json", "JSONB", "'{}'::jsonb"),
    ];

    println!(
        "\n{:<10} {:<20} {:<8} detail",
        "rz type", "PG DDL type", "reads?"
    );
    println!("{}", "-".repeat(96));
    let mut broken = vec![];
    for (rz, ddl, expr) in cases {
        let sql = format!("SELECT {expr} AS v");
        match sqlx::query(&sql).fetch_one(&pool).await {
            Ok(_) => println!("{rz:<10} {ddl:<20} {:<8} -", "OK"),
            Err(e) => {
                let msg = e.to_string();
                let msg = msg
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(60)
                    .collect::<String>();
                println!("{rz:<10} {ddl:<20} {:<8} {msg}", "FAIL");
                broken.push((rz, ddl));
            }
        }
    }

    println!(
        "\n{} of {} scalar types ruprizzle emits in Postgres DDL cannot be read back through sqlx::Any:",
        broken.len(),
        cases.len()
    );
    for (rz, ddl) in &broken {
        println!("  - {rz} -> {ddl}");
    }

    // Can a UUID be used as a bind against a uuid column?
    println!("\nbind probe: text bind compared against a uuid value");
    let r = sqlx::query("SELECT 1 WHERE '00000000-0000-0000-0000-000000000000'::uuid = $1")
        .bind("00000000-0000-0000-0000-000000000000")
        .fetch_optional(&pool)
        .await;
    match r {
        Ok(Some(_)) => println!("  OK: text bind implicitly cast to uuid"),
        Ok(None) => println!("  ran, but no row matched"),
        Err(e) => println!("  FAIL: {}", e.to_string().lines().next().unwrap_or("")),
    }

    Ok(())
}
