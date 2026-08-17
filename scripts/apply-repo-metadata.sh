#!/usr/bin/env bash
#
# Applies the canonical GitHub repository metadata.
#
# Covers MarketingPlan.md §29.1 Track A tasks 3, 4, and 5:
#   3. repo description  -> the ratified §25.4 disambiguation sentence
#   4. repo topics       -> the 11 topics listed in §29.1
#   5. repo homepage     -> must not be blank
#
# All three fields were verified EMPTY on 2026-08-17 via the public API.
#
# GATE: §29.1 task 1 requires the zero-context baseline to be recorded BEFORE
# any of this goes live. It is the only chance to capture the "before" state.
# Fill in ProjectPlan/NameResolutionBaseline.md first, then run this.
#
# Usage:
#   gh auth login                          # once
#   ./scripts/apply-repo-metadata.sh       # prompts, then applies
#   ./scripts/apply-repo-metadata.sh -n    # dry run, prints what it would do

set -euo pipefail

REPO="vaibhavgupta9877/ruprizzle-orm"

# Verbatim from ProjectPlan/CanonicalCopy.md. Do not edit here — edit there,
# then copy, so every surface keeps identical phrasing.
DESCRIPTION="ruprizzle is a schema-first ORM for Rust — a Prisma-style schema file that generates a typed client, with Drizzle-style SQL transparency and no sidecar binary."

# Consistent with the workspace Cargo.toml `homepage` field.
HOMEPAGE="https://vaibhavgupta9877.github.io/ruprizzle-orm"

TOPICS=(
  rust orm database postgres mysql sqlite sqlx prisma
  schema-first migrations type-safe
)

DRY_RUN=false
[[ "${1:-}" == "-n" || "${1:-}" == "--dry-run" ]] && DRY_RUN=true

command -v gh >/dev/null || { echo "error: gh CLI not found" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || {
  echo "error: gh is not authenticated. Run: gh auth login" >&2
  exit 1
}

echo "Repo:        $REPO"
echo "Description: $DESCRIPTION"
echo "Homepage:    $HOMEPAGE"
echo "Topics:      ${TOPICS[*]}"
echo

echo "--- current values ---"
gh api "repos/$REPO" --jq '"description: \(.description // "(empty)")\nhomepage:    \(.homepage // "(empty)")\ntopics:      \(.topics | if length == 0 then "(none)" else join(" ") end)"'
echo

if $DRY_RUN; then
  echo "dry run: no changes made."
  exit 0
fi

cat <<'GATE'
Before continuing, confirm the §28 zero-context baseline has been recorded in
ProjectPlan/NameResolutionBaseline.md. Once these fields are live the pre-change
baseline can never be measured again.
GATE
read -r -p "Baseline recorded? Apply metadata now? [y/N] " reply
[[ "$reply" == "y" || "$reply" == "Y" ]] || { echo "aborted."; exit 1; }

gh repo edit "$REPO" \
  --description "$DESCRIPTION" \
  --homepage "$HOMEPAGE"

# --add-topic is additive and idempotent; the repo currently has no topics.
topic_args=()
for t in "${TOPICS[@]}"; do topic_args+=(--add-topic "$t"); done
gh repo edit "$REPO" "${topic_args[@]}"

echo
echo "--- new values ---"
gh api "repos/$REPO" --jq '"description: \(.description)\nhomepage:    \(.homepage)\ntopics:      \(.topics | join(" "))"'
echo
echo "Done. Now tick tasks 3-5 in ProjectPlan/MarketingPlan.md §29.1 and mark"
echo "surface 2 done in ProjectPlan/CanonicalCopy.md."
