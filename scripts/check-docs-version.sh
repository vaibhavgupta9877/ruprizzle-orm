#!/usr/bin/env bash
#
# Fail when a user-facing document names an *installable* ruprizzle version that
# is not the one crates.io currently serves.
#
# Why: the v1.5.0 release attempt left quickstarts telling users to install
# `1.0.0-rc.1` long after the workspace had moved on, because nothing checked
# doc snippets against reality. `check-release-state.sh` makes the registry the
# authority on what is *published*; this script extends that authority to what
# the docs tell people to *install*.
#
# What counts as an installable-version claim:
#   ruprizzle = "1.5"                                   (Cargo.toml requirement)
#   ruprizzle-migrate = { version = "1.5.1", ... }
#   cargo install ruprizzle-cli --version 1.5.1
#   cargo add ruprizzle@1.5.1
#
# What does not:
#   "=x.y.z" exact pins — always a deliberate hold-back, not drift
#   prose mentions of a version — history and migration docs legitimately
#   discuss old versions; pinning is what misleads a new user
#
# A pin is valid iff it names the registry's current version exactly, or its
# `major.minor` line (so `1.5` stays valid across `1.5.x` releases).
#
# Files that discuss old versions on purpose are excluded: migration guides,
# the archived announcement, release history, ADRs, and audit/plan documents.
#
# Usage:
#   scripts/check-docs-version.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# --- What the registry serves ---------------------------------------------

body=$(curl -fsS -A "ruprizzle-docs-version-check" \
  "https://crates.io/api/v1/crates/ruprizzle" 2>/dev/null) || {
  echo "FAIL: cannot reach crates.io — registry state unknown, refusing to pass" >&2
  exit 1
}
served=$(printf '%s' "$body" | grep -o '"max_version":"[^"]*"' | head -1 | sed 's/.*:"//; s/"$//')
if [ -z "$served" ]; then
  echo "FAIL: crates.io returned no max_version for ruprizzle" >&2
  exit 1
fi
served_mm=${served%.*}   # major.minor of the served version

echo "crates.io serves: $served (pins must match it exactly or its $served_mm line)"
echo

# --- Files to scan ----------------------------------------------------------

# Migration guides, archived announcements, release history, ADRs and audit
# documents all name old versions on purpose.
is_excluded() {
  case "$1" in
    docs/UpgradingFromRc1.md) return 0 ;;
    docs/MigrationGuideToV1.md) return 0 ;;
    docs/announcement.md) return 0 ;;
    CHANGELOG.md | RELEASES.md) return 0 ;;
    docs/adr/* | ProjectPlan/* | ProjectAnalysis/* | local/*) return 0 ;;
    *) return 1 ;;
  esac
}

status=0

check_pin() {
  local file="$1" line="$2" ver="$3" kind="$4"
  # `=` exact pins are deliberate; skip them.
  case "$ver" in =*) return 0 ;; esac
  if [ "$ver" = "$served" ] || [ "$ver" = "$served_mm" ]; then
    return 0
  fi
  printf 'STALE  %s\n       %s\n       pins %s, but crates.io serves %s\n' \
    "$file" "$kind: $line" "$ver" "$served"
  status=1
}

while IFS= read -r file; do
  is_excluded "$file" && continue

  # Dependency requirements:  ruprizzle = "x"  /  { version = "x", ... }
  while IFS= read -r line; do
    ver=$(printf '%s' "$line" | grep -oE '"=?[0-9][^"]*"' | head -1 | tr -d '"')
    [ -n "$ver" ] && check_pin "$file" "$line" "$ver" "Cargo.toml requirement"
  done < <(grep -nE 'ruprizzle(-[a-z0-9]+)?[[:space:]]*=[[:space:]]*(\{[^}]*version[[:space:]]*=[[:space:]]*)?"=?[0-9]' "$file" | sed 's/^[0-9]*://' || true)

  # cargo install --version x
  while IFS= read -r line; do
    ver=$(printf '%s' "$line" | grep -oE '\-\-version[[:space:]]+[0-9][^[:space:]]*' | head -1 | awk '{print $2}')
    [ -n "$ver" ] && check_pin "$file" "$line" "$ver" "cargo install"
  done < <(grep -nE 'cargo install ruprizzle[a-z0-9-]*[[:space:]]+--version[[:space:]]+[0-9]' "$file" | sed 's/^[0-9]*://' || true)

  # cargo add ruprizzle@x
  while IFS= read -r line; do
    ver=$(printf '%s' "$line" | grep -oE 'ruprizzle[a-z0-9-]*@[0-9][^[:space:]"]*' | head -1 | sed 's/.*@//')
    [ -n "$ver" ] && check_pin "$file" "$line" "$ver" "cargo add"
  done < <(grep -nE 'cargo add ruprizzle[a-z0-9-]*@[0-9]' "$file" | sed 's/^[0-9]*://' || true)
done < <(git ls-files '*.md' 'llms.txt' 'crates/*/README.md' | sort -u)

echo
if [ "$status" -eq 0 ]; then
  echo "OK: every installable-version pin in the docs matches $served"
else
  echo "FAIL: docs point at versions the registry does not serve — update them or" >&2
  echo "add the file to is_excluded() if it discusses old versions on purpose" >&2
fi
exit "$status"
