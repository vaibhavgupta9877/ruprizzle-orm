#!/usr/bin/env bash
#
# Compare what the repository *claims* is published against what crates.io
# actually serves.
#
# The v1.5.0 release attempt left the tree asserting three mutually exclusive
# things at once: CHANGELOG.md said 1.5.0 was published, README.md said 1.0.1
# was, SECURITY.md said 1.0.0 was, and the registry served 1.0.0-rc.1. This
# script makes the registry the single authority, so that a published-version
# claim cannot be written by hand and left to rot.
#
# It makes network calls and is therefore NOT part of the ordinary offline
# build; run it in release and docs CI, and before writing any release notes.
#
# Usage:
#   scripts/check-release-state.sh            # report registry state
#   scripts/check-release-state.sh --expect 1.5.1
#       # additionally fail unless every crate serves exactly that version
set -euo pipefail

EXPECT=""
if [ "${1:-}" = "--expect" ]; then
  EXPECT="${2:?--expect needs a version}"
fi

# Dependency order, mirroring PUBLISH_ORDER in xtask and the publish steps in
# .github/workflows/release.yml. Keep the three in step.
CRATES=(
  ruprizzle-core
  ruprizzle-parser
  ruprizzle-dialect
  ruprizzle-macros
  ruprizzle-check
  ruprizzle-lsp
  ruprizzle
  ruprizzle-migrate
  ruprizzle-codegen
  ruprizzle-cli
  ruprizzle-turso
  ruprizzle-d1
)

workspace_version=$(
  awk '/^\[workspace\.package\]/ { in_wp = 1; next }
       /^\[/ { in_wp = 0 }
       in_wp && /^version/ { gsub(/[^0-9A-Za-z.+-]/, "", $NF); print $NF; exit }' Cargo.toml
)

echo "workspace version (Cargo.toml): $workspace_version"
echo

registry_version() {
  local crate="$1" body
  body=$(curl -fsS -A "ruprizzle-release-state-check" \
    "https://crates.io/api/v1/crates/$crate" 2>/dev/null) || {
    echo "UNPUBLISHED"
    return 0
  }
  printf '%s' "$body" |
    grep -o '"max_version":"[^"]*"' |
    head -1 |
    sed 's/.*:"//; s/"$//'
}

status=0
printf '%-22s %s\n' "CRATE" "LATEST ON CRATES.IO"
printf '%-22s %s\n' "-----" "-------------------"
for crate in "${CRATES[@]}"; do
  v=$(registry_version "$crate")
  printf '%-22s %s\n' "$crate" "$v"
  if [ -n "$EXPECT" ] && [ "$v" != "$EXPECT" ]; then
    status=1
  fi
  # Be polite to the registry.
  sleep 1
done

echo
if [ -n "$EXPECT" ]; then
  if [ "$status" -eq 0 ]; then
    echo "OK: every crate serves $EXPECT"
  else
    echo "FAIL: not every crate serves $EXPECT — do not claim $EXPECT is released" >&2
  fi
  exit "$status"
fi

cat <<'NOTE'
Reminder: the only version that may be described as "published", "released" or
"latest" in README.md, docs/README.md, CHANGELOG.md, RELEASES.md, SECURITY.md,
llms.txt or any crate README is one this command reports above. A git tag is not
a release; a green CHANGELOG entry is not a release.
NOTE
