#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
  printf 'macOS kit validation failed: %s\n' "$1" >&2
  exit 1
}

case "$(uname -m)" in
  arm64) platform="macos-arm64" ;;
  x86_64) platform="macos-x64" ;;
  *) fail "unsupported host architecture: $(uname -m)" ;;
esac

CLI_SOURCE="${HARNESS_CLI_FIXTURE:-$ROOT_DIR/scripts/bin/harness-cli}"
if [ ! -x "$CLI_SOURCE" ] && [ -z "${HARNESS_CLI_FIXTURE:-}" ]; then
  CLI_SOURCE="$ROOT_DIR/target/debug/harness-cli"
fi
test -x "$CLI_SOURCE" || fail "Harness CLI fixture is unavailable; run cargo build -p harness-cli first"

SYMPHONY_SOURCE="${HARNESS_SYMPHONY_FIXTURE:-$ROOT_DIR/target/release/harness-symphony}"
if [ ! -x "$SYMPHONY_SOURCE" ] && [ -z "${HARNESS_SYMPHONY_FIXTURE:-}" ]; then
  SYMPHONY_SOURCE="$ROOT_DIR/target/debug/harness-symphony"
fi
test -x "$SYMPHONY_SOURCE" || \
  fail "Harness Symphony fixture is unavailable; run cargo build -p harness-symphony first"

OUT_DIR="$TMP_DIR/out"
"$ROOT_DIR/scripts/build-harness-macos-kit.sh" \
  --platform "$platform" \
  --cli "$CLI_SOURCE" \
  --symphony "$SYMPHONY_SOURCE" \
  --out-dir "$OUT_DIR"

ARCHIVE="$OUT_DIR/harness-macos-${platform#macos-}.tar.gz"
test -f "$ARCHIVE" || fail "kit archive was not created"

KIT_DIR="$TMP_DIR/kit"
mkdir -p "$KIT_DIR"
tar -xzf "$ARCHIVE" -C "$KIT_DIR"

FORMULA_PREFIX="$TMP_DIR/formula"
mkdir -p "$FORMULA_PREFIX/bin"
ln -s "$KIT_DIR/bin/harness" "$FORMULA_PREFIX/bin/harness"
HARNESS="$FORMULA_PREFIX/bin/harness"
test -x "$HARNESS" || fail "kit launcher is missing or not executable"
"$HARNESS" --help >/dev/null
test "$("$HARNESS" --version)" = "$(tr -d '\r\n' < "$ROOT_DIR/scripts/harness-kit-version")" || \
  fail "kit launcher reported the wrong version"

test -x "$KIT_DIR/bin/harness-symphony" || fail "kit archive is missing the Symphony launcher"
test -x "$KIT_DIR/libexec/harness-kit/scripts/bin/harness-symphony" || \
  fail "kit archive is missing the Symphony runner binary"
test -f "$KIT_DIR/libexec/harness-kit/scripts/bin/harness-symphony.sha256" || \
  fail "kit archive is missing the Symphony runner checksum"
test -f "$KIT_DIR/libexec/harness-kit/web-ui-dist/index.html" || \
  fail "kit archive is missing the Symphony web UI dist"
ln -s "$KIT_DIR/bin/harness-symphony" "$FORMULA_PREFIX/bin/harness-symphony"
"$FORMULA_PREFIX/bin/harness-symphony" --help >/dev/null || \
  fail "packaged Symphony runner does not execute"

TARGET="$TMP_DIR/project"
"$HARNESS" init --dry-run --yes "$TARGET" >/dev/null
"$HARNESS" --init --dry-run --yes "$TARGET" >/dev/null
"$HARNESS" init --yes "$TARGET" >/dev/null
"$HARNESS" update --dry-run --yes "$TARGET" >/dev/null
test -x "$TARGET/scripts/bin/harness-cli" || fail "kit init did not install the local CLI"

rm "$KIT_DIR/libexec/harness-kit/scripts/bin/harness-cli.sha256"
if "$HARNESS" init --yes "$TMP_DIR/checksum-target" >"$TMP_DIR/checksum-error.txt" 2>&1; then
  fail "kit init accepted a missing local CLI checksum"
fi
grep -Fq 'Local Harness CLI checksum file is missing' "$TMP_DIR/checksum-error.txt" || \
  fail "kit init did not explain the missing local CLI checksum"

FORMULA="$TMP_DIR/harness.rb"
"$ROOT_DIR/scripts/render-homebrew-formula.sh" \
  --kit-version 0.1.0 \
  --base-url "file://$OUT_DIR" \
  --arm-sha 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --intel-sha fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210 \
  --output "$FORMULA"
ruby -c "$FORMULA" >/dev/null
grep -Fq 'harness-macos-arm64.tar.gz' "$FORMULA" || \
  fail "Formula omitted the arm64 kit archive"
grep -Fq 'harness-macos-x64.tar.gz' "$FORMULA" || \
  fail "Formula omitted the Intel kit archive"
grep -Fq 'bin/harness-symphony' "$FORMULA" || \
  fail "Formula omitted the Symphony runner symlink"

KIT_WORKFLOW="$ROOT_DIR/.github/workflows/harness-kit-release.yml"
test -f "$KIT_WORKFLOW" || fail "kit release workflow is missing"
ruby -e 'require "yaml"; YAML.load_file(ARGV.fetch(0))' "$KIT_WORKFLOW"
rg -Fq 'cargo build -p harness-cli' "$KIT_WORKFLOW" || \
  fail "kit release verification does not build the CLI fixture"
rg -Fq 'cargo build --release -p harness-symphony' "$KIT_WORKFLOW" || \
  fail "kit release workflow does not build the Symphony runner"
for release_workflow in \
  "$ROOT_DIR/.github/workflows/harness-cli-release.yml" \
  "$KIT_WORKFLOW" \
  "$ROOT_DIR/.github/workflows/post-merge-maintenance.yml"
do
  test -f "$release_workflow" || fail "release workflow is missing: $release_workflow"
  if rg -n 'uses: [^[:space:]]+@(v[0-9]|stable)' "$release_workflow" >/dev/null; then
    fail "release workflow uses a mutable GitHub Action reference: $release_workflow"
  fi
done
for publish_workflow in \
  "$ROOT_DIR/.github/workflows/harness-cli-release.yml" \
  "$KIT_WORKFLOW"
do
  if rg -n '^  push:' "$publish_workflow" >/dev/null; then
    fail "release workflow has a duplicate tag push trigger: $publish_workflow"
  fi
done
grep -Fq 'harness-macos-arm64.tar.gz' "$KIT_WORKFLOW" || \
  fail "kit release workflow omitted the arm64 archive"
grep -Fq 'harness-macos-x64.tar.gz' "$KIT_WORKFLOW" || \
  fail "kit release workflow omitted the Intel archive"
grep -Fq 'HOMEBREW_TAP_TOKEN' "$KIT_WORKFLOW" || \
  fail "kit release workflow omitted the tap publishing credential"
rg -Uq 'name: Publish kit release\n    needs: build\n    runs-on: macos-15' "$KIT_WORKFLOW" || \
  fail "kit release workflow does not test the tap on macOS"
rg -Fq 'brew install winterzxzz/tap/harness' "$KIT_WORKFLOW" || \
  fail "kit release workflow does not install the rendered Formula"
rg -Fq 'harness init --dry-run --yes' "$KIT_WORKFLOW" || \
  fail "kit release workflow does not smoke-test harness init"

rg -Fq 'brew install winterzxzz/tap/harness' "$ROOT_DIR/README.md" || \
  fail "README omitted the Homebrew installation command"
rg -Fq 'brew bundle' "$ROOT_DIR/README.md" || \
  fail "README omitted the multi-Mac Brewfile flow"
rg -Fq 'harness update' "$ROOT_DIR/README.md" || \
  fail "README omitted the project update command"
rg -Fq 'scripts/bin/harness-cli' "$ROOT_DIR/README.md" || \
  fail "README omitted the repository-local agent CLI contract"

printf 'macOS kit validation passed\n'
