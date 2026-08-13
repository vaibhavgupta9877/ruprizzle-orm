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
RUST_BACKTRACE=0 RUPRIZZLE_SOAK_DURATION_SECONDS=0 cargo mutants -p ruprizzle --jobs 4 --minimum-test-timeout 5 --output local/mutants-runtime
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

`cargo mutants -p ruprizzle --list` reports **1004 mutants** in the runtime crate.

A full run with the soak test disabled (`RUPRIZZLE_SOAK_DURATION_SECONDS=0`) was
started with:

```bash
RUST_BACKTRACE=0 RUPRIZZLE_SOAK_DURATION_SECONDS=0 cargo mutants -p ruprizzle --jobs 4 --minimum-test-timeout 5 --output local/mutants-runtime
```

The auto-set test timeout rose to ~95 s per mutant because the runtime test
suite compiles and runs several integration test binaries per mutant, even with
soak disabled. At 1004 mutants the full run would take roughly two wall-clock
hours, so it is left as a follow-up activity:

```bash
RUST_BACKTRACE=0 RUPRIZZLE_SOAK_DURATION_SECONDS=0 cargo mutants -p ruprizzle --jobs 4 --minimum-test-timeout 30 --output local/mutants-runtime
```

The `crates/migrate` baseline is the current recorded score.

## Next steps

1. Inspect `local/mutants/caught.txt` and `local/mutants/missed.txt` (or the
   equivalent `mutants.json`) for the per-function survival list.
2. Add targeted assertions or snapshot tests for the `missed` mutants before
   adding more features.
3. Re-run `cargo mutants` after improving tests and update this baseline.
