#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
MANIFEST="$ROOT_DIR/scripts/harness-install-files.txt"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
  printf 'install payload validation failed: %s\n' "$1" >&2
  exit 1
}

require_entry() {
  grep -Fxq "$1" "$MANIFEST" || fail "required payload entry missing: $1"
}

reject_pattern() {
  if grep -Eq "$1" "$MANIFEST"; then
    fail "forbidden payload pattern present: $1"
  fi
}

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

for required in \
  AGENTS.md \
  docs/ARCHITECTURE.md \
  docs/CONTEXT_RULES.md \
  docs/FEATURE_INTAKE.md \
  docs/HARNESS.md \
  docs/HARNESS_BACKLOG.md \
  docs/TEST_MATRIX.md \
  docs/decisions/README.md \
  docs/product/README.md \
  docs/stories/README.md \
  docs/stories/backlog.md \
  docs/templates/decision.md \
  scripts/README.md \
  .gitignore
do
  require_entry "$required"
done

reject_pattern '^README\.md$'
reject_pattern '^docs/decisions/[0-9][0-9][0-9][0-9].*\.md$'
reject_pattern '^docs/stories/(US-|epics/)'
reject_pattern '^docs/superpowers/'
reject_pattern '^\.harness/'
reject_pattern '^\.symphony/'
reject_pattern '^\.worktrees/'
reject_pattern '^harness\.db(-wal|-shm)?$'

grep -Fq 'PAYLOAD_MANIFEST="scripts/harness-install-files.txt"' \
  "$ROOT_DIR/scripts/install-harness.sh" || fail "Bash installer does not use the shared manifest"
grep -Fq '$script:PayloadManifest = "scripts/harness-install-files.txt"' \
  "$ROOT_DIR/scripts/install-harness.ps1" || fail "PowerShell installer does not use the shared manifest"

bash -n "$ROOT_DIR/scripts/install-harness.sh"

DRY_TARGET="$TMP_DIR/dry-target"
DRY_OUTPUT="$TMP_DIR/dry-run.txt"
"$ROOT_DIR/scripts/install-harness.sh" \
  --directory "$DRY_TARGET" --yes --dry-run >"$DRY_OUTPUT"

grep -Fq 'create   docs/HARNESS.md' "$DRY_OUTPUT" || fail "dry run omitted core Harness policy"
grep -Fq 'create   docs/decisions/README.md' "$DRY_OUTPUT" || fail "dry run omitted decision scaffold"
if grep -Eq '^(create|update|skip|overwrite)[[:space:]]+README\.md([[:space:]]|$)' "$DRY_OUTPUT"; then
  fail "dry run includes source repository README"
fi
if grep -Eq 'docs/decisions/[0-9][0-9][0-9][0-9].*\.md' "$DRY_OUTPUT"; then
  fail "dry run includes numbered decision history"
fi

RELEASE_DIR="$TMP_DIR/release"
mkdir -p "$RELEASE_DIR"
CLI_SOURCE="$ROOT_DIR/scripts/bin/harness-cli"
if [ ! -x "$CLI_SOURCE" ]; then
  CLI_SOURCE="$ROOT_DIR/target/debug/harness-cli"
fi
test -x "$CLI_SOURCE" || fail "Harness CLI fixture is unavailable; run cargo build first"
cp "$CLI_SOURCE" "$RELEASE_DIR/harness-cli-test-platform"
sha256_file "$RELEASE_DIR/harness-cli-test-platform" > \
  "$RELEASE_DIR/harness-cli-test-platform.sha256"

FRESH_TARGET="$TMP_DIR/fresh-target"
HARNESS_CLI_PLATFORM=test-platform \
HARNESS_CLI_BASE_URL="file://$RELEASE_DIR" \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$FRESH_TARGET" --yes >/dev/null

test -f "$FRESH_TARGET/docs/HARNESS.md" || fail "fresh install omitted core policy"
test -f "$FRESH_TARGET/docs/decisions/README.md" || fail "fresh install omitted decision scaffold"
test ! -e "$FRESH_TARGET/README.md" || fail "fresh install copied source repository README"
if find "$FRESH_TARGET/docs/decisions" -maxdepth 1 -type f -name '[0-9][0-9][0-9][0-9]*.md' | grep -q .; then
  fail "fresh install copied numbered decision history"
fi
test ! -e "$FRESH_TARGET/harness.db" || fail "fresh install created operational database"
test ! -e "$FRESH_TARGET/.harness" || fail "fresh install created runtime history"

MERGE_TARGET="$TMP_DIR/merge-target"
mkdir -p "$MERGE_TARGET/docs/decisions"
printf 'target readme\n' >"$MERGE_TARGET/README.md"
printf 'target decision\n' >"$MERGE_TARGET/docs/decisions/0001-target.md"
HARNESS_CLI_PLATFORM=test-platform \
HARNESS_CLI_BASE_URL="file://$RELEASE_DIR" \
  "$ROOT_DIR/scripts/install-harness.sh" \
  --directory "$MERGE_TARGET" --merge --yes >/dev/null

test "$(cat "$MERGE_TARGET/README.md")" = 'target readme' || fail "merge changed target README"
test "$(cat "$MERGE_TARGET/docs/decisions/0001-target.md")" = 'target decision' || \
  fail "merge changed target decision history"

printf 'install payload validation passed\n'
