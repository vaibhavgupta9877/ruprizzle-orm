# Performance

The end-to-end benchmark compares ruprizzle against hand-written `sqlx::query`
for the same operations, because ruprizzle sits on top of `sqlx`. The
interesting number is therefore our overhead, not our speed versus another ORM
on different hardware.

Run it with:

```bash
RUPRIZZLE_TEST_PG_URL=postgres://ruprizzle:ruprizzle@localhost:5432/ruprizzle_test \
  cargo bench -p ruprizzle --bench end_to_end
```

If no database is reachable, the bench skips automatically so `cargo bench`
still works offline.

## P8-02 thresholds and measured results

Measured on a single workstation (Intel Core Ultra 7 265K, 20 logical cores,
32 GB RAM) against a local PostgreSQL database using `sqlx::Any`.

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
single/1 000-row overhead is likely dominated by `sqlx::Any` text marshalling
and the extra row-by-row decoding in the ORM path; the 2-level include grouping
has improved substantially. P8-02 measures this overhead against the thresholds;
actually optimising the remaining per-row decode cost belongs to a separate work
item.

## Text-marshalling cost

`sqlx::Any` serialises `Uuid`, `Decimal`, `DateTime`, `Date`, `Time` and `Json`
to text on every outbound bind and parses them from text on every inbound row.
That cost is real and unquantified in the current numbers because the bench
schema uses `BIGINT` and `TEXT` columns. In a real workload with rich types the
gap between ruprizzle and hand-written sqlx is expected to be dominated by this
`Any` driver behaviour, not by the ORM layer. If you profile, compare against
hand-written `sqlx::query` using the same `Any` driver; switching to a
driver-specific pool would remove this cost for both paths.
