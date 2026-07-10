#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: render-homebrew-formula.sh --kit-version <x.y.z> --arm-sha <sha256> --intel-sha <sha256> --output <path> [--base-url <url>]
EOF
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

valid_sha256() {
  [ "${#1}" -eq 64 ] && [[ "$1" != *[!0-9a-fA-F]* ]]
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
TEMPLATE="$ROOT_DIR/packaging/homebrew/Formula/harness.rb.tmpl"
KIT_VERSION=""
KIT_BASE_URL=""
ARM_SHA256=""
INTEL_SHA256=""
OUTPUT=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --kit-version)
      [ "$#" -ge 2 ] || fail "--kit-version requires a value"
      KIT_VERSION="$2"
      shift 2
      ;;
    --base-url)
      [ "$#" -ge 2 ] || fail "--base-url requires a value"
      KIT_BASE_URL="${2%/}"
      shift 2
      ;;
    --arm-sha)
      [ "$#" -ge 2 ] || fail "--arm-sha requires a checksum"
      ARM_SHA256="$2"
      shift 2
      ;;
    --intel-sha)
      [ "$#" -ge 2 ] || fail "--intel-sha requires a checksum"
      INTEL_SHA256="$2"
      shift 2
      ;;
    --output)
      [ "$#" -ge 2 ] || fail "--output requires a path"
      OUTPUT="$2"
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

case "$KIT_VERSION" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) fail "--kit-version must be a semantic version" ;;
esac
if [ -z "$KIT_BASE_URL" ]; then
  KIT_BASE_URL="https://github.com/winterzxzz/repository-harness/releases/download/harness-kit-v$KIT_VERSION"
fi
case "$KIT_BASE_URL" in
  https://github.com/winterzxzz/repository-harness/releases/download/*|file:///*) ;;
  *) fail "--base-url must be a Harness GitHub release URL or absolute file:// URL" ;;
esac
valid_sha256 "$ARM_SHA256" || fail "--arm-sha must be a 64-character hexadecimal SHA-256"
valid_sha256 "$INTEL_SHA256" || fail "--intel-sha must be a 64-character hexadecimal SHA-256"
[ -f "$TEMPLATE" ] || fail "Formula template is missing: $TEMPLATE"
[ -n "$OUTPUT" ] || fail "--output is required"

mkdir -p "$(dirname "$OUTPUT")"
TMP_OUTPUT="$(mktemp "${OUTPUT}.tmp.XXXXXX")"
awk \
  -v kit_version="$KIT_VERSION" \
  -v kit_base_url="$KIT_BASE_URL" \
  -v arm_sha="$ARM_SHA256" \
  -v intel_sha="$INTEL_SHA256" \
  '{
    gsub(/@KIT_VERSION@/, kit_version)
    gsub(/@KIT_BASE_URL@/, kit_base_url)
    gsub(/@ARM_SHA256@/, arm_sha)
    gsub(/@INTEL_SHA256@/, intel_sha)
    print
  }' "$TEMPLATE" > "$TMP_OUTPUT"
mv "$TMP_OUTPUT" "$OUTPUT"
