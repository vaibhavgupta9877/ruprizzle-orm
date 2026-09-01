# Agent notes

## Build target directory

- This workspace is configured to use `G:\cargo-target` as the shared build
  target by default (see `.cargo/config.toml`).  This keeps the checkout on `D:`
  small and centralizes build artifacts.
- You can override the location at any time with the `CARGO_TARGET_DIR`
  environment variable.
- To free build artifacts for this project, run `cargo clean` from the workspace
  root.  This will remove the directory configured as `target-dir`.

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
  ```powershell
  $env:RUST_BENCH_DRIVER="rusqlite"
  cargo run --example cross_orm_bench -p ruprizzle --release --features sqlite-rusqlite
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
  cargo doc --workspace --no-deps --all-features
  cargo xtask harden
  ```

- SQLite migration property (multi-change):
  ```powershell
  $env:PROPTEST_CASES='100'; cargo test -p ruprizzle-deep-tests --test migrate_sqlite_roundtrip
  ```

## Branching model

- `dev-main` is the integration branch and the base for **all** development work.
  Unless the user says otherwise, branch from `dev-main` and merge back into it.
- Version lines (`dev-v1-x`, `dev-v2-x`, ...) and feature branches merge **into**
  `dev-main`. They never merge into `main` directly.
- `main` is the release line. It only ever receives a merge from `dev-main`, at
  publish time, together with the version bump, the changelog entry and the tag.
- Publishing to crates.io happens from `main` after that merge, never from
  `dev-main` and never from a feature branch.

```
feature/*  ->  dev-vN-x  ->  dev-main  --(release)-->  main  --> crates.io
```
