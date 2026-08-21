# Agent notes

## Useful commands

- Run the full cross-ORM benchmark suite:
  ```
  python local/cross-orm-bench/run_bench.py
  ```
  This builds the `cross_orm_bench` example (plus the `prax`, `sea-orm`, and
  `diesel` harnesses), runs Node harnesses for Drizzle/Prisma, and updates
  `local/cross-orm-bench/{raw_results.json,results.json,BENCHMARKS.log}` and
  `docs/BenchmarkResults.md`.

- Run a single `rusqlite` benchmark trial manually:
  ```
  cargo build --example cross_orm_bench -p ruprizzle --release --features sqlite-rusqlite
  $env:RUST_BENCH_DRIVER="rusqlite"
  .\target\release\examples\cross_orm_bench.exe
  ```

- Run the ruprizzle test suite (including rusqlite tests):
  ```powershell
  $env:RUPRIZZLE_TEST_RUSQLITE=1
  cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'
  ```

- Run the native `rusqlite` soak test (resumable, 48-hour gate is **waived**; the
  scripts remain available for optional extended validation):
  ```powershell
  # One segment (1 hour by default; set RUPRIZZLE_SOAK_DURATION_SECONDS to override).
  .\local\run-soak-segment.ps1

  # Repeat the above command until it prints `soak finished`, or run the loop
  # that starts 1-hour segments back-to-back until completed.
  .\local\run-soak-48h.ps1

  # State is kept in `local/soak-48h/soak-rusqlite.db`.
  # The 48-hour W4-02 gate has been waived after 15.56 h / 1.46 B ops / 0 errors.
  # Use these scripts only if you want additional optional soak evidence.
  ```

## Verification commands

- Mechanical gates:
  ```powershell
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  $env:RUPRIZZLE_TEST_RUSQLITE=1; cargo test -p ruprizzle --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite'
  cargo doc --workspace --no-deps
  cargo xtask harden
  ```

- SQLite migration property (multi-change):
  ```powershell
  $env:PROPTEST_CASES='100'; cargo test -p ruprizzle-deep-tests --test migrate_sqlite_roundtrip
  ```

## Default branch

- The repository default branch is now `dev-v0-2`. It was created from
  `perf/research-harnesses` after merging `w2-phase`, `w3-phase`, `w4-phase`,
  and `w5-phase` with `--no-ff`. Unless the user says otherwise or manually
  changes branches, use `dev-v0-2` as the base for all future work.
