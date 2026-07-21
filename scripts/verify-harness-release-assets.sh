#!/usr/bin/env bash
set -euo pipefail

[[ $# == 1 ]] || {
  echo "usage: $0 <artifact-directory>" >&2
  exit 2
}

artifact_dir=$1
[[ -d "$artifact_dir" ]] || {
  echo "Harness core release artifact directory is missing: $artifact_dir" >&2
  exit 1
}

expected=$(printf '%s\n' \
  harness-linux-arm64 harness-linux-arm64.sha256 \
  harness-linux-x64 harness-linux-x64.sha256 \
  harness-macos-arm64 harness-macos-arm64.sha256 \
  harness-macos-x64 harness-macos-x64.sha256 \
  harness-windows-x64.exe harness-windows-x64.exe.sha256)
actual=$(find "$artifact_dir" -maxdepth 1 -type f -exec basename {} \; | LC_ALL=C sort)

if [[ "$actual" != "$expected" ]]; then
  echo "Harness core release artifact inventory differs:" >&2
  diff -u <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") >&2 || true
  exit 1
fi

echo "Harness core release artifact inventory passed: 5 binaries and 5 checksums"
