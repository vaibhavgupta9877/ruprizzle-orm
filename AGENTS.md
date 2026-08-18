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

- Run the native `rusqlite` soak test:
  ```powershell
  $env:RUPRIZZLE_TEST_RUSQLITE=1
  $env:RUPRIZZLE_SOAK_DURATION_SECONDS=3600
  $env:RUPRIZZLE_SOAK_WORKERS=8
  cargo test -p ruprizzle --test soak --features 'sqlite-rusqlite,ruprizzle-testkit/sqlite-rusqlite' --release -- sqlite --nocapture
  ```

## Default branch

- The repository default branch is now `dev-v0-2`. It was created from
  `perf/research-harnesses` after merging `w2-phase`, `w3-phase`, `w4-phase`,
  and `w5-phase` with `--no-ff`. Unless the user says otherwise or manually
  changes branches, use `dev-v0-2` as the base for all future work.
