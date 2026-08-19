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

- Run the native `rusqlite` soak test (resumable, segmented 48-hour gate):
  ```powershell
  # One segment (1 hour by default; set RUPRIZZLE_SOAK_DURATION_SECONDS to override).
  .\local\run-soak-segment.ps1

  # Repeat the above command until it prints `soak finished`, or run the loop
  # that starts 1-hour segments back-to-back until completed.
  .\local\run-soak-48h.ps1

  # State is kept in `local/soak-48h/soak-rusqlite.db`.
  ```

## Default branch

- The repository default branch is now `dev-v0-2`. It was created from
  `perf/research-harnesses` after merging `w2-phase`, `w3-phase`, `w4-phase`,
  and `w5-phase` with `--no-ff`. Unless the user says otherwise or manually
  changes branches, use `dev-v0-2` as the base for all future work.
