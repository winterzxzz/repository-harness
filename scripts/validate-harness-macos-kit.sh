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

CLI_SOURCE="$ROOT_DIR/scripts/bin/harness-cli"
if [ ! -x "$CLI_SOURCE" ]; then
  CLI_SOURCE="$ROOT_DIR/target/debug/harness-cli"
fi
test -x "$CLI_SOURCE" || fail "Harness CLI fixture is unavailable; run cargo build -p harness-cli first"

OUT_DIR="$TMP_DIR/out"
"$ROOT_DIR/scripts/build-harness-macos-kit.sh" \
  --platform "$platform" \
  --cli "$CLI_SOURCE" \
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

printf 'macOS kit validation passed\n'
