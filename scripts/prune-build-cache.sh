#!/usr/bin/env bash
# Reclaim disk from the shared cargo build cache and the crate registry.
#
# The shared `build.target-dir` (see CONTRIBUTING.md) accumulates artifacts from
# every project and every toolchain that has ever built against it, and nothing
# removes them on its own: cargo's own cache GC is still nightly-only as of
# 1.95.  Run this when the cache outgrows its budget; it never touches sources.
#
#   scripts/prune-build-cache.sh              # drop artifacts unused for 30 days
#   DAYS=7 scripts/prune-build-cache.sh       # more aggressive
#   MAXSIZE=15GB scripts/prune-build-cache.sh # then trim to a hard ceiling
#   DRY_RUN=1 scripts/prune-build-cache.sh    # show what would go
set -euo pipefail

cd "$(dirname "$0")/.."

DAYS="${DAYS:-30}"
MAXSIZE="${MAXSIZE:-}"
sweep_flags=()
[[ -n "${DRY_RUN:-}" ]] && sweep_flags+=(--dry-run)

if ! command -v cargo-sweep >/dev/null 2>&1; then
  echo "cargo-sweep not installed; run: cargo binstall cargo-sweep" >&2
  exit 1
fi

# `cargo metadata` resolves target-dir exactly as a build would, honouring
# CARGO_TARGET_DIR, the repo config, and ~/.cargo/config.toml in the right
# order.  (`cargo config get` would be more direct but is still nightly-only.)
target_dir="$(
  cargo metadata --format-version 1 --no-deps |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])'
)"
echo "build cache: ${target_dir}"
du -sh "$target_dir"

# cargo-sweep takes a *project* path and resolves the target directory itself,
# so running it from the repo root prunes the whole shared cache -- including
# artifacts other projects left there, which is the point of sharing it.
cargo sweep "${sweep_flags[@]}" --time "$DAYS" .

# Anything built by a toolchain rustup no longer has is dead weight; this is
# what piles up after a rust-toolchain.toml bump.
cargo sweep "${sweep_flags[@]}" --installed .

# A hard ceiling, evicting oldest-first, for when age alone is not enough.
[[ -n "$MAXSIZE" ]] && cargo sweep "${sweep_flags[@]}" --maxsize "$MAXSIZE" .

echo "after:"
du -sh "$target_dir"

# The registry keeps a compressed .crate plus its extracted source for every
# version ever resolved.  The extracted copies are re-created on demand from the
# archives, so they are the cheap thing to drop.
registry_src="${CARGO_HOME:-$HOME/.cargo}/registry/src"
if [[ -d "$registry_src" ]]; then
  echo "registry sources (re-extracted on demand): $(du -sh "$registry_src" | cut -f1)"
  echo "  remove with: rm -rf ${registry_src}"
fi
