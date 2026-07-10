#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build-harness-macos-kit.sh --platform <macos-arm64|macos-x64> --cli <path> --out-dir <path>
EOF
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

sha256_file() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{ print $1 }'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{ print $1 }'
  else
    fail "shasum or sha256sum is required to package the Harness CLI"
  fi
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PLATFORM=""
CLI_SOURCE=""
OUT_DIR=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --platform)
      [ "$#" -ge 2 ] || fail "--platform requires a value"
      PLATFORM="$2"
      shift 2
      ;;
    --cli)
      [ "$#" -ge 2 ] || fail "--cli requires a path"
      CLI_SOURCE="$2"
      shift 2
      ;;
    --out-dir)
      [ "$#" -ge 2 ] || fail "--out-dir requires a path"
      OUT_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "Unknown option: $1"
      ;;
  esac
done

case "$PLATFORM" in
  macos-arm64|macos-x64) ;;
  *) fail "--platform must be macos-arm64 or macos-x64" ;;
esac
[ -n "$CLI_SOURCE" ] || fail "--cli is required"
[ -x "$CLI_SOURCE" ] || fail "Harness CLI binary is not executable: $CLI_SOURCE"
[ -n "$OUT_DIR" ] || fail "--out-dir is required"

STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGE_DIR"' EXIT
KIT_ROOT="$STAGE_DIR/libexec/harness-kit"
mkdir -p "$STAGE_DIR/bin" "$KIT_ROOT/scripts/bin"

copy_required_file() {
  local relative="$1"
  local source="$ROOT_DIR/$relative"
  local target="$KIT_ROOT/$relative"

  [ -f "$source" ] || fail "Required kit file is missing: $source"
  mkdir -p "$(dirname "$target")"
  cp -p "$source" "$target"
}

copy_required_file "AGENTS.md"
copy_required_file ".gitignore"
copy_required_file "scripts/install-harness.sh"
copy_required_file "scripts/harness-install-files.txt"
copy_required_file "scripts/harness-cli-release-tag"
copy_required_file "scripts/harness-kit-version"
copy_required_file "scripts/harness-upstream-repository"

while IFS= read -r relative || [ -n "$relative" ]; do
  case "$relative" in
    ""|\#*) continue ;;
  esac
  copy_required_file "$relative"
done < "$ROOT_DIR/scripts/harness-install-files.txt"

for schema in "$ROOT_DIR"/scripts/schema/*.sql; do
  [ -f "$schema" ] || continue
  copy_required_file "scripts/schema/$(basename "$schema")"
done

cp -p "$ROOT_DIR/scripts/harness" "$STAGE_DIR/bin/harness"
chmod 755 "$STAGE_DIR/bin/harness"
cp -p "$CLI_SOURCE" "$KIT_ROOT/scripts/bin/harness-cli"
chmod 755 "$KIT_ROOT/scripts/bin/harness-cli"
sha256_file "$KIT_ROOT/scripts/bin/harness-cli" > "$KIT_ROOT/scripts/bin/harness-cli.sha256"

mkdir -p "$OUT_DIR"
ARCHIVE="$OUT_DIR/harness-macos-${PLATFORM#macos-}.tar.gz"
tar -C "$STAGE_DIR" -czf "$ARCHIVE" bin libexec
printf '%s\n' "$ARCHIVE"
