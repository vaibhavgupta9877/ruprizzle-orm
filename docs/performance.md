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
32 GB RAM) against a local PostgreSQL database.

| Benchmark | Hand-written sqlx | ruprizzle | Acceptance | Status |
|---|---|---|---|---|
| single-row select by PK | 87.0 µs | 91.5 µs | within 5% | **exceeds** (+5.2%) |
| 1 000-row select | 719.6 µs | 718.9 µs | within 5% | within threshold |
| 2-level include (100 parents × 10 children × 10 grandchildren) | 7.38 ms | 9.78 ms | within 15% | **exceeds** (+32.5%) |
| bulk insert 10 000 rows | — | — | within 10% | **not measured** |

The bulk-insert case could not be completed because the local PostgreSQL
instance, which uses a tmpfs-backed data directory for the integration suite,
ran out of WAL space (`could not write to file "pg_wal/xlogtemp...": No space
left on device`) during the 10 000-row insert and became unreachable for new
connections afterwards.

The 1 000-row case is within the 5% threshold; the single-row case is 5.2%
slower, just outside the target, so it is not at parity on this run. The 2-level
include is the only case that currently shows a meaningful overhead; the likely
contributors are the extra in-Rust decoding and the attachment/grouping step.
P8-02 measures this overhead against the thresholds; actually optimising the
2-level include grouping belongs to a separate work item (for example, P5-03).

## Text-marshalling cost

`sqlx::Any` serialises `Uuid`, `Decimal`, `DateTime`, `Date`, `Time` and `Json`
to text on every outbound bind and parses them from text on every inbound row.
That cost is real and unquantified in the current numbers because the bench
schema uses `BIGINT` and `TEXT` columns. In a real workload with rich types the
gap between ruprizzle and hand-written sqlx is expected to be dominated by this
`Any` driver behaviour, not by the ORM layer. If you profile, compare against
hand-written `sqlx::query` using the same `Any` driver; switching to a
driver-specific pool would remove this cost for both paths.
