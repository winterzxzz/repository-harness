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
  CLAUDE.md \
  docs/ARCHITECTURE.md \
  docs/CONTEXT_RULES.md \
  docs/FEATURE_INTAKE.md \
  docs/HARNESS.md \
  docs/SYMPHONY_QUICKSTART.md \
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
grep -Fq "\$script:PayloadManifest = \"scripts/harness-install-files.txt\"" \
  "$ROOT_DIR/scripts/install-harness.ps1" || fail "PowerShell installer does not use the shared manifest"
grep -Fq 'function Assert-ManagedPathSafe' \
  "$ROOT_DIR/scripts/install-harness.ps1" || fail "PowerShell installer lacks managed-path safety checks"
grep -Fq "Assert-ManagedPathSafe \$file" \
  "$ROOT_DIR/scripts/install-harness.ps1" || fail "PowerShell installer does not preflight managed paths"
for runtime_rule in '.symphony/' '.worktrees/' '!.harness/' '.harness/*' \
  '!.harness/changesets/' '!.harness/changesets/*.changeset.jsonl'
do
  grep -Fq "\"$runtime_rule\"" "$ROOT_DIR/scripts/install-harness.ps1" || \
    fail "PowerShell installer omits runtime ignore rule $runtime_rule"
done

bash -n "$ROOT_DIR/scripts/install-harness.sh"
"$ROOT_DIR/scripts/validate-harness-cli-release.sh"

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
test ! -e "$DRY_TARGET/.harness/install-state.tsv" || \
  fail "dry run created managed-file state"

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

IGNORE_TARGET="$TMP_DIR/ignore-target"
mkdir -p "$IGNORE_TARGET"
printf '%s\n' 'vendor/' '.harness/' >"$IGNORE_TARGET/.gitignore"
for _ in 1 2; do
  HARNESS_CLI_PLATFORM=test-platform \
  HARNESS_CLI_BASE_URL="file://$RELEASE_DIR" \
    "$ROOT_DIR/scripts/install-harness.sh" --directory "$IGNORE_TARGET" --merge --yes >/dev/null
done
HARNESS_CLI_PLATFORM=test-platform \
HARNESS_CLI_BASE_URL="file://$RELEASE_DIR" \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$IGNORE_TARGET" --override --force --yes >/dev/null
for rule in '.symphony/' '.worktrees/' '!.harness/' '.harness/*' \
  '!.harness/changesets/' '!.harness/changesets/*.changeset.jsonl'
do
  test "$(grep -Fxc "$rule" "$IGNORE_TARGET/.gitignore")" -eq 1 || \
    fail "existing target .gitignore must contain exactly one $rule rule"
done
grep -Fqx 'vendor/' "$IGNORE_TARGET/.gitignore" || \
  fail "installer replaced target-owned .gitignore content"
git -C "$IGNORE_TARGET" init -q
mkdir -p "$IGNORE_TARGET/.symphony" "$IGNORE_TARGET/.harness/runs/run_1" \
  "$IGNORE_TARGET/.harness/changesets"
: >"$IGNORE_TARGET/.symphony/state.db"
: >"$IGNORE_TARGET/.harness/runs/run_1/RESULT.json"
: >"$IGNORE_TARGET/.harness/changesets/run_1.changeset.jsonl"
git -C "$IGNORE_TARGET" check-ignore -q .symphony/state.db || \
  fail "Symphony state is not ignored"
git -C "$IGNORE_TARGET" check-ignore -q .harness/runs/run_1/RESULT.json || \
  fail "run evidence is not ignored"
if git -C "$IGNORE_TARGET" check-ignore -q .harness/changesets/run_1.changeset.jsonl; then
  fail "changesets must remain visible to Git"
fi

test -f "$FRESH_TARGET/docs/HARNESS.md" || fail "fresh install omitted core policy"
test -f "$FRESH_TARGET/docs/decisions/README.md" || fail "fresh install omitted decision scaffold"
test -f "$FRESH_TARGET/AGENTS.md" || fail "fresh install omitted AGENTS.md"
test -f "$FRESH_TARGET/docs/SYMPHONY_QUICKSTART.md" || \
  fail "fresh install omitted Symphony Quickstart"
grep -Fq 'command -v harness-symphony' "$FRESH_TARGET/docs/SYMPHONY_QUICKSTART.md" || \
  fail "fresh install Quickstart does not verify the installed Symphony command"
if grep -Fq 'target/debug/harness-symphony' "$FRESH_TARGET/docs/SYMPHONY_QUICKSTART.md"; then
  fail "fresh install Quickstart relies on a source-only Symphony build path"
fi
for command in 'runs start' 'runs heartbeat' 'runs complete'; do
  grep -Fq "$command" "$FRESH_TARGET/AGENTS.md" || \
    fail "fresh install AGENTS.md omits external lifecycle command: $command"
  grep -Fq "$command" "$FRESH_TARGET/docs/SYMPHONY_QUICKSTART.md" || \
    fail "fresh install Quickstart omits external lifecycle command: $command"
done
test -f "$FRESH_TARGET/CLAUDE.md" || fail "fresh install omitted CLAUDE.md"
grep -Fq '@AGENTS.md' "$FRESH_TARGET/CLAUDE.md" || \
  fail "fresh install CLAUDE.md does not import AGENTS.md"
grep -Fq '## Template Review Boundary' "$FRESH_TARGET/AGENTS.md" || \
  fail "fresh install omitted the template review boundary"
grep -Fq 'scripts/harness-install-files.txt' "$FRESH_TARGET/AGENTS.md" || \
  fail "template review boundary does not identify the source manifest"
grep -Fq 'harness-symphony run <story-id>' "$FRESH_TARGET/AGENTS.md" || \
  fail "fresh install AGENTS.md does not route approved story execution through Symphony"
grep -Fq 'Do not pass `--no-web`' "$FRESH_TARGET/AGENTS.md" || \
  fail "fresh install AGENTS.md does not preserve Symphony Web UI startup"
grep -Fq 'HARNESS_RUN_ID' "$FRESH_TARGET/AGENTS.md" || \
  fail "fresh install AGENTS.md does not prevent nested Symphony runs"
if grep -Fq '.codex/skills/harness-intake-griller/SKILL.md' "$FRESH_TARGET/AGENTS.md"; then
  fail "fresh install AGENTS.md references an uninstalled project skill"
fi

REFRESH_AGENT_TARGET="$TMP_DIR/refresh-agent-target"
mkdir -p "$REFRESH_AGENT_TARGET"
printf '%s\n' \
  '# Project Agent Instructions' \
  '' \
  '<!-- HARNESS:BEGIN -->' \
  'stale Harness guidance' \
  '<!-- HARNESS:END -->' \
  >"$REFRESH_AGENT_TARGET/AGENTS.md"
HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
  "$ROOT_DIR/scripts/install-harness.sh" \
  --directory "$REFRESH_AGENT_TARGET" --merge --refresh-agent-shim --yes >/dev/null
grep -Fq 'harness-symphony run <story-id>' "$REFRESH_AGENT_TARGET/AGENTS.md" || \
  fail "refreshed AGENTS.md does not route approved story execution through Symphony"
grep -Fq 'Do not pass `--no-web`' "$REFRESH_AGENT_TARGET/AGENTS.md" || \
  fail "refreshed AGENTS.md does not preserve Symphony Web UI startup"
grep -Fq 'HARNESS_RUN_ID' "$REFRESH_AGENT_TARGET/AGENTS.md" || \
  fail "refreshed AGENTS.md does not prevent nested Symphony runs"
for command in 'runs start' 'runs heartbeat' 'runs complete'; do
  grep -Fq "$command" "$REFRESH_AGENT_TARGET/AGENTS.md" || \
    fail "refreshed AGENTS.md omits external lifecycle command: $command"
done
test ! -e "$FRESH_TARGET/README.md" || fail "fresh install copied source repository README"
for decision_file in "$FRESH_TARGET/docs/decisions"/[0-9][0-9][0-9][0-9]*.md; do
  test ! -f "$decision_file" || fail "fresh install copied numbered decision history"
done
test ! -e "$FRESH_TARGET/harness.db" || fail "fresh install created operational database"
test -f "$FRESH_TARGET/.harness/install-state.tsv" || \
  fail "fresh install omitted managed-file state"
test ! -e "$FRESH_TARGET/.harness/changesets" || \
  fail "fresh install created runtime history"
if grep -Eq '^\| US-[0-9]+' "$FRESH_TARGET/docs/TEST_MATRIX.md"; then
  fail "fresh install copied source story evidence into the test matrix"
fi
test -f "$FRESH_TARGET/docs/HARNESS_MATURITY.md" || \
  fail "fresh install omitted Harness maturity guidance"
test -f "$FRESH_TARGET/docs/HARNESS_COMPONENTS.md" || \
  fail "fresh install omitted Harness component guidance"
test -f "$FRESH_TARGET/docs/README.md" || \
  fail "fresh install omitted the documentation map"
test -f "$FRESH_TARGET/docs/product/README.md" || \
  fail "fresh install omitted the product documentation scaffold"
if grep -Fq 'repository-harness' "$FRESH_TARGET/docs/HARNESS_MATURITY.md" || \
   grep -Fq 'repository-harness' "$FRESH_TARGET/docs/HARNESS_COMPONENTS.md"; then
  fail "fresh install copied source repository identity into operating docs"
fi
if grep -Eq 'SYMPHONY_(QUICKSTART|SCOPE)\.md' "$FRESH_TARGET/docs/README.md"; then
  fail "fresh install documentation map references omitted Symphony docs"
fi
if grep -Fq 'symphony-web-ui-controller.md' "$FRESH_TARGET/docs/product/README.md"; then
  fail "fresh install product index references an omitted source contract"
fi
if grep -Eq 'Phase [0-9]+ pins `harness-cli-v' "$FRESH_TARGET/scripts/README.md"; then
  fail "fresh install scripts guide contains a stale source release claim"
fi
if grep -Fq 'docs/decisions/0004-sqlite-durable-layer.md' "$FRESH_TARGET/docs/CONTEXT_RULES.md" || \
   grep -Fq 'docs/decisions/0005-prebuilt-rust-harness-cli.md' "$FRESH_TARGET/docs/CONTEXT_RULES.md"; then
  fail "fresh install context rules reference source-only decision history"
fi

LOCAL_CLI_TARGET="$TMP_DIR/local-cli-target"
HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
HARNESS_CLI_BASE_URL="file://$TMP_DIR/missing-cli-release" \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$LOCAL_CLI_TARGET" --yes >/dev/null

cmp "$CLI_SOURCE" "$LOCAL_CLI_TARGET/scripts/bin/harness-cli" || \
  fail "local CLI source was not copied"

UPDATE_SOURCE="$TMP_DIR/update-source"
UPDATE_TARGET="$TMP_DIR/update-target"
mkdir -p "$UPDATE_SOURCE/scripts"
cp "$ROOT_DIR/scripts/install-harness.sh" "$UPDATE_SOURCE/scripts/install-harness.sh"
cp "$ROOT_DIR/scripts/harness-install-files.txt" "$UPDATE_SOURCE/scripts/harness-install-files.txt"
cp "$ROOT_DIR/scripts/harness-cli-release-tag" "$UPDATE_SOURCE/scripts/harness-cli-release-tag"
cp "$ROOT_DIR/scripts/harness-kit-version" "$UPDATE_SOURCE/scripts/harness-kit-version"
cp "$ROOT_DIR/scripts/harness-upstream-repository" "$UPDATE_SOURCE/scripts/harness-upstream-repository"
while IFS= read -r relative || [ -n "$relative" ]; do
  case "$relative" in
    ""|\#*) continue ;;
  esac
  mkdir -p "$(dirname "$UPDATE_SOURCE/$relative")"
  cp "$ROOT_DIR/$relative" "$UPDATE_SOURCE/$relative"
done < "$ROOT_DIR/scripts/harness-install-files.txt"
mkdir -p "$UPDATE_SOURCE/scripts/schema"
cp "$ROOT_DIR"/scripts/schema/*.sql "$UPDATE_SOURCE/scripts/schema/"

HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --yes >/dev/null

printf '\nHarness source revision one\n' >> "$UPDATE_SOURCE/docs/HARNESS.md"
HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --update --yes >/dev/null
grep -Fq 'Harness source revision one' "$UPDATE_TARGET/docs/HARNESS.md" || \
  fail "update did not replace an unchanged managed file"

printf '\nProject-local customization\n' >> "$UPDATE_TARGET/docs/HARNESS.md"
printf '\nHarness source revision two\n' >> "$UPDATE_SOURCE/docs/HARNESS.md"
HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --update --yes >/dev/null
grep -Fq 'Project-local customization' "$UPDATE_TARGET/docs/HARNESS.md" || \
  fail "update overwrote a locally modified managed file"
if grep -Fq 'Harness source revision two' "$UPDATE_TARGET/docs/HARNESS.md"; then
  fail "update applied a newer source revision over local customization"
fi

HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --update --yes --force >/dev/null
grep -Fq 'Harness source revision two' "$UPDATE_TARGET/docs/HARNESS.md" || \
  fail "forced update did not replace a locally modified managed file"
find "$UPDATE_TARGET/.harness-backup" -path '*/docs/HARNESS.md' -type f | \
  grep -q . || fail "forced update omitted a backup for the modified managed file"

rm "$UPDATE_TARGET/.harness/install-state.tsv"
if HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
  HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
  HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --update --yes \
  >"$TMP_DIR/legacy-update.txt" 2>&1; then
  fail "legacy update ran without explicit adoption"
fi
grep -Fq "harness update --adopt" "$TMP_DIR/legacy-update.txt" || \
  fail "legacy update did not explain how to adopt existing Harness files"

HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --update --adopt --yes >/dev/null
test -f "$UPDATE_TARGET/.harness/install-state.tsv" || \
  fail "adoption did not create managed-file state"

SYMLINK_TARGET="$TMP_DIR/symlink-target.txt"
printf 'outside project\n' > "$SYMLINK_TARGET"
rm "$UPDATE_TARGET/docs/HARNESS.md"
ln -s "$SYMLINK_TARGET" "$UPDATE_TARGET/docs/HARNESS.md"
if HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
  HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
  HARNESS_CLI_PLATFORM=test-platform \
  "$UPDATE_SOURCE/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --update --force --yes \
  >"$TMP_DIR/symlink-update.txt" 2>&1; then
  fail "forced update accepted a symlinked managed file"
fi
test "$(cat "$SYMLINK_TARGET")" = 'outside project' || \
  fail "forced update wrote through a managed-file symlink"

INITIAL_SYMLINK_TARGET="$TMP_DIR/initial-symlink-target"
INITIAL_SYMLINK_OUTSIDE="$TMP_DIR/initial-symlink-outside.txt"
mkdir -p "$INITIAL_SYMLINK_TARGET"
printf 'outside project\n' >"$INITIAL_SYMLINK_OUTSIDE"
ln -s "$INITIAL_SYMLINK_OUTSIDE" "$INITIAL_SYMLINK_TARGET/.gitignore"
if HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
  HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
  HARNESS_CLI_PLATFORM=test-platform \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$INITIAL_SYMLINK_TARGET" --force --yes \
  >"$TMP_DIR/initial-symlink-install.txt" 2>&1; then
  fail "initial force install accepted a symlinked managed file"
fi
test "$(cat "$INITIAL_SYMLINK_OUTSIDE")" = 'outside project' || \
  fail "initial force install wrote through a managed-file symlink"

STATE_SYMLINK_TARGET="$TMP_DIR/state-symlink-target"
STATE_SYMLINK_OUTSIDE="$TMP_DIR/state-symlink-outside"
mkdir -p "$STATE_SYMLINK_TARGET" "$STATE_SYMLINK_OUTSIDE"
ln -s "$STATE_SYMLINK_OUTSIDE" "$STATE_SYMLINK_TARGET/.harness"
if HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
  HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
  HARNESS_CLI_PLATFORM=test-platform \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$STATE_SYMLINK_TARGET" --force --yes \
  >"$TMP_DIR/state-symlink-install.txt" 2>&1; then
  fail "initial install accepted a symlinked Harness state directory"
fi
test ! -e "$STATE_SYMLINK_OUTSIDE/install-state.tsv" || \
  fail "initial install wrote through a symlinked Harness state directory"

BACKUP_SYMLINK_TARGET="$TMP_DIR/backup-symlink-target"
BACKUP_SYMLINK_OUTSIDE="$TMP_DIR/backup-symlink-outside"
mkdir -p "$BACKUP_SYMLINK_TARGET" "$BACKUP_SYMLINK_OUTSIDE"
ln -s "$BACKUP_SYMLINK_OUTSIDE" "$BACKUP_SYMLINK_TARGET/.harness-backup"
if HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
  HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
  HARNESS_CLI_PLATFORM=test-platform \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$BACKUP_SYMLINK_TARGET" --force --yes \
  >"$TMP_DIR/backup-symlink-install.txt" 2>&1; then
  fail "initial install accepted a symlinked Harness backup directory"
fi
test ! -e "$BACKUP_SYMLINK_OUTSIDE/AGENTS.md" || \
  fail "initial install wrote through a symlinked Harness backup directory"

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

if rg -n 'hoangnb24/repository-harness' \
  "$ROOT_DIR/README.md" \
  "$ROOT_DIR/scripts/install-harness.sh" \
  "$ROOT_DIR/scripts/install-harness.ps1" \
  "$ROOT_DIR/scripts/README.md" >/dev/null; then
  fail "legacy upstream repository identifier remains in public installer surfaces"
fi

printf 'install payload validation passed\n'
