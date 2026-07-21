#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
temp=$(mktemp -d)
trap 'rm -rf "$temp"' EXIT
ref=harness-cli-v0.1.14
source="$temp/source"
assets="$temp/assets/$ref"
consumer="$temp/consumer"
platform=fixture-platform
mkdir -p "$source/scripts/schema" "$assets" "$consumer"
printf '%s\n' 'AGENTS.md' >"$source/scripts/harness-install-files.txt"
printf '%s\n' '# fixture compatibility files' >"$source/scripts/harness-cli-install-files.txt"
printf '%s\n' '# fixture agents' >"$source/AGENTS.md"
printf '%s\n' 'SELECT 1;' >"$source/scripts/schema/001-fixture.sql"
printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$assets/harness-cli-$platform"
chmod 755 "$assets/harness-cli-$platform"
(cd "$assets" && shasum -a 256 "harness-cli-$platform" >"harness-cli-$platform.sha256")

HARNESS_SOURCE_BASE_URL="file://$source" \
HARNESS_CLI_BASE_URL="file://$assets" \
HARNESS_CLI_PLATFORM="$platform" \
HARNESS_CLI_RELEASE_TAG="$ref" \
  "$root/scripts/install-harness.sh" --directory "$consumer" --with-cli --yes >/dev/null

git -C "$consumer" init -q
git -C "$consumer" config user.name fixture
git -C "$consumer" config user.email fixture@example.invalid
mkdir -p "$consumer/.harness/changesets"
printf '%s\n' '{"op":"changeset.header","version":1,"run_id":"consumer-fixture","base_schema_version":13}' \
  >"$consumer/.harness/changesets/consumer-fixture.changeset.jsonl"
git -C "$consumer" add .gitignore .harness/changesets/consumer-fixture.changeset.jsonl
git -C "$consumer" ls-files --error-unmatch .harness/changesets/consumer-fixture.changeset.jsonl >/dev/null
git -C "$consumer" check-ignore .harness/changesets/consumer-fixture.changeset.jsonl >"$temp/ignored.out" 2>&1 && {
  echo "fresh consumer changeset was ignored" >&2
  exit 1
}

echo "fresh consumer can track its own semantic changeset"
