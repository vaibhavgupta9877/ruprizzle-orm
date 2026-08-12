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
  ```
  cargo test -p ruprizzle --features sqlite-rusqlite
  ```
