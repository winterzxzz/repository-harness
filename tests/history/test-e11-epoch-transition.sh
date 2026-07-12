#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cli="$root/target/debug/harness-cli"
transition="$root/scripts/harness-epoch-transition.py"
temp=$(mktemp -d)
trap 'rm -rf "$temp"' EXIT

cargo build --quiet --manifest-path "$root/Cargo.toml" -p harness-cli

make_fixture() {
  local name=$1
  local fixture="$temp/$name"
  mkdir -p "$fixture/scripts" "$fixture/.harness/changesets" \
    "$fixture/prepared/changesets" "$fixture/archive"
  cp -R "$root/scripts/schema" "$fixture/scripts/schema"
  HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/harness.db" "$cli" init >/dev/null
  HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/prepared/harness.db" "$cli" init >/dev/null
  sqlite3 "$fixture/harness.db" "CREATE TABLE epoch_sentinel(value TEXT); INSERT INTO epoch_sentinel VALUES('legacy');"
  sqlite3 "$fixture/prepared/harness.db" "CREATE TABLE epoch_sentinel(value TEXT); INSERT INTO epoch_sentinel VALUES('fresh');"
  printf 'legacy-log\n' >"$fixture/.harness/changesets/legacy.fixture.jsonl"
  printf 'fresh-log\n' >"$fixture/prepared/changesets/fresh.fixture.jsonl"
  printf '%s\n' "$fixture"
}

assert_fenced() {
  local fixture=$1 reads_allowed=${2:-0} before=missing after=missing
  if [[ -f "$fixture/harness.db" ]] && sqlite3 "$fixture/harness.db" \
      "SELECT 1 FROM sqlite_master WHERE type='table' AND name='intake';" | grep -q 1; then
    before=$(sqlite3 "$fixture/harness.db" 'SELECT COUNT(*) FROM intake;')
  fi
  if [[ "$reads_allowed" == 1 ]]; then
    HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/harness.db" \
      "$cli" query stats >/dev/null
  elif HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/harness.db" \
      "$cli" query stats >"$fixture/read.out" 2>&1; then
    echo "read startup unexpectedly accepted a mixed epoch pair" >&2
    exit 1
  else
    grep -Fq 'writes remain fenced' "$fixture/read.out"
  fi
  if HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/harness.db" \
    "$cli" intake --type change_request --summary fenced --lane tiny \
      >"$fixture/write.out" 2>&1; then
    echo "mutation unexpectedly crossed an incomplete epoch journal" >&2
    exit 1
  fi
  grep -Fq 'writes remain fenced' "$fixture/write.out"
  if [[ -f "$fixture/harness.db" ]] && sqlite3 "$fixture/harness.db" \
      "SELECT 1 FROM sqlite_master WHERE type='table' AND name='intake';" | grep -q 1; then
    after=$(sqlite3 "$fixture/harness.db" 'SELECT COUNT(*) FROM intake;')
  fi
  [[ "$before" == "$after" ]]
}

begin_with_crash() {
  local fixture=$1 step=$2
  if "$transition" --repo-root "$fixture" begin \
      --transition-id "transition-$step" \
      --fresh-db "$fixture/prepared/harness.db" \
      --fresh-log "$fixture/prepared/changesets" \
      --archive-root "$fixture/archive" \
      --inject-after "$step" >"$fixture/begin.out" 2>&1; then
    echo "crash injection unexpectedly completed at $step" >&2
    exit 1
  fi
  grep -Fq "injected crash after $step" "$fixture/begin.out"
}

for step in prepared legacy_db_archived legacy_log_archived fresh_db_activated fresh_log_activated; do
  fixture=$(make_fixture "forward-$step")
  begin_with_crash "$fixture" "$step"
  assert_fenced "$fixture" 0
  "$transition" --repo-root "$fixture" recover --strategy forward
  assert_fenced "$fixture" 1
  [[ "$(sqlite3 "$fixture/harness.db" 'SELECT value FROM epoch_sentinel;')" == fresh ]]
  [[ -f "$fixture/.harness/changesets/fresh.fixture.jsonl" ]]
  [[ -f "$fixture/archive/legacy-harness.db" ]]
  [[ -f "$fixture/archive/legacy-changesets/legacy.fixture.jsonl" ]]
  "$transition" --repo-root "$fixture" complete --transition-id "transition-$step"
  HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/harness.db" \
    "$cli" intake --type change_request --summary unfenced --lane tiny >/dev/null
done

for step in prepared legacy_db_archived legacy_log_archived fresh_db_activated fresh_log_activated; do
  fixture=$(make_fixture "compensate-$step")
  begin_with_crash "$fixture" "$step"
  assert_fenced "$fixture" 0
  "$transition" --repo-root "$fixture" recover --strategy compensate
  [[ "$(sqlite3 "$fixture/harness.db" 'SELECT value FROM epoch_sentinel;')" == legacy ]]
  [[ -f "$fixture/.harness/changesets/legacy.fixture.jsonl" ]]
  [[ -f "$fixture/prepared/harness.db" ]]
  [[ -f "$fixture/prepared/changesets/fresh.fixture.jsonl" ]]
  HARNESS_REPO_ROOT="$fixture" HARNESS_DB_PATH="$fixture/harness.db" \
    "$cli" intake --type change_request --summary compensated --lane tiny >/dev/null
done

# Journal tampering must never unlock mutation.
fixture=$(make_fixture tamper)
begin_with_crash "$fixture" prepared
python3 - "$fixture/.harness/epoch-transition/journal.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
document["payload"]["state"] = "complete"
path.write_text(json.dumps(document))
PY
assert_fenced "$fixture" 0

echo "epoch transition crash, recovery, compensation, checksum, and writer-fence tests passed"
