#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

fail() {
  printf 'architecture validation failed: %s\n' "$1" >&2
  exit 1
}

if grep -n 'crate::infrastructure' "$ROOT_DIR/crates/harness-cli/src/application.rs" >/dev/null; then
  fail "application layer imports infrastructure directly"
fi

if grep -E -n '^use crate::(application|interface|infrastructure)' "$ROOT_DIR/crates/harness-cli/src/domain.rs" >/dev/null; then
  fail "domain layer imports an outer layer"
fi

printf 'architecture validation passed\n'
