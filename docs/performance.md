# Performance

The end-to-end benchmark compares ruprizzle against hand-written `sqlx::query`
for the same operations, because ruprizzle sits on top of `sqlx`. The
interesting number is therefore our overhead, not our speed versus another ORM
on different hardware.

The benchmark uses `ruprizzle::connect_with` for the ruprizzle side and a
matching native `sqlx::Postgres` pool for the hand-written baseline. This is the
same path a normal `postgres://` URL takes by default, and it avoids the
additional text-marshalling cost of the generic `sqlx::Any` driver.

Run it with:

```bash
RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo bench -p ruprizzle --bench end_to_end
```

If no database is reachable, the bench skips automatically so `cargo bench`
still works offline.

## Full-table include fast path

Unfiltered parent queries use a fast path that loads the entire child table in
one query instead of building a large `IN (...)` list. To avoid unbounded
materialisation on huge child tables, `ruprizzle` first `COUNT(*)`s the child
table and only takes the fast path when the count is below
`PoolConfig::full_table_include_limit` (default `100_000`). Larger child tables
fall back to a chunked `IN (...)` path, preserving the one-query-per-level
guarantee without decoding millions of rows into memory. Set the field to `None`
to disable the fast path entirely.

## P8-02 thresholds and measured results

Measured on a single workstation (Intel Core Ultra 7 265K, 20 logical cores,
32 GB RAM) against a local PostgreSQL database using native `sqlx::Postgres`
(via `ruprizzle::connect_with`).

| Benchmark | Hand-written sqlx | ruprizzle | Acceptance | Status |
|---|---|---|---|---|
| single-row select by PK | 49.9 µs | 53.5 µs | within 5% | **exceeds** (+7.2%) |
| 1 000-row select | 602.3 µs | 674.7 µs | within 5% | **exceeds** (+12.0%) |
| 2-level include (100 parents × 10 children × 10 grandchildren) | 7.14 ms | 7.84 ms | within 15% | within threshold (+9.8%) |
| bulk insert 10 000 rows | 35.4 ms | 36.0 ms | within 10% | within threshold (+1.8%) |

The bulk-insert case now completes successfully on the local PostgreSQL
instance; the previous run was blocked by a tmpfs WAL-space exhaustion.

On this run the 1 000-row and single-row selects are both above the 5% parity
target. The 2-level include and bulk insert are within their thresholds. The
single/1 000-row overhead is likely dominated by the extra row-by-row decoding
in the ORM path; the 2-level include grouping has improved substantially.
P8-02 measures this overhead against the thresholds; actually optimising the
remaining per-row decode cost belongs to a separate work item.

## Prepared statements

`SelectQuery::prepare()` compiles a `SELECT` once and returns a `PreparedSelect` that
re-uses the compiled SQL. Bind values can be swapped with `bind` or `bind_many` and
the statement can be executed again without recompiling.

Measured with `cargo bench -p ruprizzle --features sqlite-rusqlite --bench query_construction`
on the same workstation:

| Benchmark | Time |
|---|---|
| `to_sql_select_by_pk` | 614 ns |
| `prepare_select_by_pk` | 553 ns |
| `prepared_rebind_select_by_pk` | 53 ns |

Compiling the SQL is ~600 ns; rebinding a prepared statement is an order of
magnitude cheaper. For query shapes that are executed repeatedly with different
parameters, `prepare()` removes that per-call compilation cost.

## Text-marshalling cost

`sqlx::Any` serialises `Uuid`, `Decimal`, `DateTime`, `Date`, `Time` and `Json`
to text on every outbound bind and parses them from text on every inbound row.
That cost is real, but the default `postgres://` connection path uses the native
`sqlx::Postgres` driver and binds rich types directly, so most Postgres users
are not affected. The cost only applies if you explicitly construct
`Pool::Any(...)` or use a generic `Any` URL.

On SQLite, enable the `sqlite-rusqlite` feature and connect with
`?driver=rusqlite` in the URL. This bypasses `sqlx::Any` text marshalling for
`Uuid`, `Decimal`, `DateTime`, `Date`, `Time` and `Json` and decodes them from
the native SQLite value directly. It is still stored as text in the database, so
exact `Decimal` arithmetic should use `Int` minor units or a PostgreSQL backend. The synchronous `rusqlite` call is offloaded to `tokio::task::spawn_blocking` so it does not pin the async runtime's worker threads; this trades a small per-call dispatch cost for runtime responsiveness.

In a real workload with rich types the gap between ruprizzle and hand-written
sqlx is expected to be dominated by the ORM decode path, not by driver
marshalling, as long as the native driver is selected. If you profile, compare
against hand-written `sqlx::query` using the same native `sqlx::Postgres` pool.

## See also

- [Benchmark results](BenchmarkResults.md) — cross-ORM SQLite numbers for
  ruprizzle (sqlx), ruprizzle (rusqlite), prax, Sea-ORM, Diesel, Prisma, and
  Drizzle, including the new query-construction operations added in the latest
  benchmark run.
- [Feature master comparison](FeaturesMasterComparison.md) — full feature and
  architecture comparison, including the advanced SQL builder feature matrix.
