# Mutation testing baseline

This project uses [`cargo-mutants`](https://mutants.rs/) to find code that the
existing tests do not actually assert on. The baseline is recorded here so the
W4 hardening gate can be measured.

## Configuration

The shared configuration lives in `.cargo/mutants.toml`:

- `test_tool = "cargo"`
- `build_timeout_multiplier = 3.0`
- `cap_lints = true`
- `output = "local/mutants"`

Skip the long-running soak test for the runtime crate by setting
`RUPRIZZLE_SOAK_DURATION_SECONDS=0` before invoking `cargo mutants`.

## Reproducing

```bash
# Count mutants for a crate
cargo mutants -p ruprizzle-migrate --list
cargo mutants -p ruprizzle --list

# Run the baseline for a crate
RUST_BACKTRACE=0 RUPRIZZLE_SOAK_DURATION_SECONDS=0 cargo mutants -p ruprizzle-migrate --jobs 4 --minimum-test-timeout 5
RUST_BACKTRACE=0 RUPRIZZLE_SOAK_DURATION_SECONDS=0 cargo mutants -p ruprizzle --jobs 4 --minimum-test-timeout 30 --output local/mutants-runtime
```

## Baseline results

### `crates/migrate` (`ruprizzle-migrate`)

Run: `cargo mutants -p ruprizzle-migrate --jobs 4 --minimum-test-timeout 5`

| Metric | Count |
|--------|-------|
| Mutants generated | 393 |
| Caught (killed) | 99 |
| Missed (survived) | 251 |
| Unviable | 32 |
| Timeouts | 11 |
| Mutation score | ~25.2% |

A re-run on 2026-08-17 with the current source found:

| Metric | Count |
|--------|-------|
| Mutants generated | 606 |
| Caught (killed) | 14 |
| Missed (survived) | 33 |
| Unviable | 557 |
| Timeouts | 2 |
| Mutation score | **~28.6 %** (14 / 49) |

The high `missed` count means many unit tests in `crates/migrate` are passing
without asserting on the value being produced. Examples of surviving mutants
include:

- `Change::is_destructive` returning `true` or `false` unconditionally.
- `Change::description` returning `String::new()` or a nonsense string.
- `diff_enums`, `diff_columns`, `diff_relations`, `diff_indexes`, etc. returning
  empty vectors.
- `split_statements` arithmetic and comparison operators flipped without test
  failure.
- `Migrator::apply_all` body replaced with `Ok(Default::default())`.

These are the W4 finding #10 sites where 218 existing tests pass without
asserting the actual behaviour.

### `crates/runtime` (`ruprizzle`)

`cargo mutants -p ruprizzle --list` currently reports **1684 mutants** in the
runtime crate (the previous 1004 count was from an earlier feature set).

A full local run is expensive: the default copy-per-mutant mode recompiles the
runtime integration tests for each mutant (many hours on a single-core path),
and `--in-place` on Windows can fail to overwrite mapped source files. The
baseline is therefore generated in CI (`.github/workflows/mutants.yml`), which
shards the runtime run across four parallel jobs.

A local unsharded run was started on 2026-08-17 with:

```bash
RUST_BACKTRACE=0 RUPRIZZLE_SOAK_DURATION_SECONDS=0 cargo mutants -p ruprizzle --jobs 4 --minimum-test-timeout 30 --output local/mutants-runtime
```

The live log is at `local/mutants-runtime.log`. Results will be recorded here
once the run completes.

## Known gap: `ruprizzle-migrate` mutation coverage

`ruprizzle-migrate` currently has very poor mutation coverage. A local run
started on 2026-08-17 shows a large number of surviving mutants in
`Change::description`, `diff_enums`, `diff_columns`, `diff_relations`,
`diff_indexes`, `split_statements`, and `Migrator::apply_all`. The live log is
at `local/mutants-migrate.log`. Fixing this is deferred to the W6 hardening
cycle; for the `0.4.0-beta.1` milestone it is documented as a known gap.

## Next steps

1. Inspect `local/mutants/caught.txt` and `local/mutants/missed.txt` (or the
   equivalent `mutants.json`) for the per-function survival list.
2. Add targeted assertions or snapshot tests for the `missed` mutants before
   adding more features.
3. Re-run `cargo mutants` after improving tests and update this baseline.
