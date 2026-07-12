#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
ARTIFACT_DIR=""
REMOTE=false
REMOTE_REPOSITORY="${GITHUB_REPOSITORY:-}"

fail() {
  printf 'Harness CLI release validation failed: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: validate-harness-cli-release.sh [options]

Validate the versioned Harness CLI release pin and optional local artifacts.

Options:
      --artifact-dir <path>  Validate CLI binaries and SHA-256 files in a directory.
      --remote               Verify the pinned GitHub Release and all platform assets exist.
      --repo <owner/repo>    GitHub repository used by --remote.
  -h, --help                 Show this help.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --artifact-dir)
      [ "$#" -ge 2 ] || fail "$1 requires a directory"
      ARTIFACT_DIR="$2"
      shift 2
      ;;
    --remote)
      REMOTE=true
      shift
      ;;
    --repo)
      [ "$#" -ge 2 ] || fail "$1 requires owner/repository"
      REMOTE_REPOSITORY="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

tag_file="$ROOT_DIR/scripts/harness-cli-release-tag"
[ -f "$tag_file" ] || fail "release tag file is missing: $tag_file"

version="$({
  awk '
    /^\[package\]$/ { in_package=1; next }
    /^\[/ { in_package=0 }
    in_package && /^name = "harness-cli"$/ { found_name=1; next }
    in_package && found_name && /^version = "/ {
      value=$0
      sub(/^version = "/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' "$ROOT_DIR/crates/harness-cli/Cargo.toml"
})"
[ -n "$version" ] || fail "could not read harness-cli version from Cargo.toml"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || \
  fail "harness-cli version is not stable semver: $version"

tag="$(awk 'NF && $1 !~ /^#/ { print $1; exit }' "$tag_file")"
[ -n "$tag" ] || fail "release tag file is empty: $tag_file"
expected_tag="harness-cli-v$version"
[ "$tag" = "$expected_tag" ] || \
  fail "$tag_file pins $tag but Cargo.toml declares $expected_tag"

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{ print $1 }'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{ print $1 }'
  else
    fail "shasum or sha256sum is required to validate CLI artifacts"
  fi
}

if [ -n "$ARTIFACT_DIR" ]; then
  [ -d "$ARTIFACT_DIR" ] || fail "artifact directory does not exist: $ARTIFACT_DIR"
  artifact_count=0
  for artifact in "$ARTIFACT_DIR"/harness-cli-*; do
    [ -f "$artifact" ] || continue
    case "$artifact" in
      *.sha256) continue ;;
    esac
    artifact_count=$((artifact_count + 1))
    artifact_name="$(basename "$artifact")"
    case "$artifact_name" in
      harness-cli-macos-arm64|harness-cli-macos-x64|harness-cli-linux-x64|harness-cli-linux-arm64|harness-cli-windows-x64.exe)
        ;;
      *)
        fail "unexpected CLI artifact name: $artifact_name"
        ;;
    esac
    checksum="$artifact.sha256"
    [ -f "$checksum" ] || fail "checksum file is missing: $checksum"
    expected_checksum="$(awk 'NF { print $1; exit }' "$checksum")"
    [[ "$expected_checksum" =~ ^[[:xdigit:]]{64}$ ]] || \
      fail "checksum file is invalid: $checksum"
    actual_checksum="$(sha256_file "$artifact")"
    expected_checksum="$(printf '%s' "$expected_checksum" | tr '[:upper:]' '[:lower:]')"
    actual_checksum="$(printf '%s' "$actual_checksum" | tr '[:upper:]' '[:lower:]')"
    [ "$expected_checksum" = "$actual_checksum" ] || \
      fail "checksum mismatch for $artifact_name"
  done
  [ "$artifact_count" -gt 0 ] || fail "no CLI artifacts found in $ARTIFACT_DIR"
fi

if [ "$REMOTE" = true ]; then
  command -v gh >/dev/null 2>&1 || fail "gh CLI is required for --remote"
  [ -n "$REMOTE_REPOSITORY" ] || fail "--repo or GITHUB_REPOSITORY is required for --remote"
  if ! gh release view "$tag" --repo "$REMOTE_REPOSITORY" >/dev/null 2>&1; then
    fail "GitHub Release $tag is not published in $REMOTE_REPOSITORY; publish the CLI release before building a kit"
  fi
  assets="$(gh release view "$tag" --repo "$REMOTE_REPOSITORY" --json assets --jq '.assets[].name')"
  for asset in \
    harness-cli-macos-arm64 \
    harness-cli-macos-arm64.sha256 \
    harness-cli-macos-x64 \
    harness-cli-macos-x64.sha256 \
    harness-cli-linux-x64 \
    harness-cli-linux-x64.sha256 \
    harness-cli-linux-arm64 \
    harness-cli-linux-arm64.sha256 \
    harness-cli-windows-x64.exe \
    harness-cli-windows-x64.exe.sha256
  do
    grep -Fxq "$asset" <<<"$assets" || \
      fail "GitHub Release $tag is missing asset $asset"
  done
fi

printf 'Harness CLI release validation passed: %s\n' "$tag"
