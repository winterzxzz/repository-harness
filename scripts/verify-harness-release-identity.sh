#!/usr/bin/env bash
set -euo pipefail

[[ $# == 4 ]] || {
  echo "usage: $0 <pretag|tagged> <harness-vX.Y.Z> <source-sha> <proof-run-id>" >&2
  exit 2
}

mode=$1
tag=$2
source_sha=$3
proof_run=$4
case "$mode" in pretag|tagged) ;; *) echo "release identity rejected: invalid mode" >&2; exit 2 ;; esac
[[ "$tag" =~ ^harness-v([0-9]+\.[0-9]+\.[0-9]+)$ ]] || {
  echo "release identity rejected: invalid stable tag: $tag" >&2
  exit 1
}
expected_version=${BASH_REMATCH[1]}
[[ "$source_sha" =~ ^[0-9a-f]{40}$ ]] || { echo "release identity rejected: invalid source SHA" >&2; exit 1; }
[[ "$proof_run" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "release identity rejected: invalid proof run" >&2; exit 1; }

head_sha=$(git rev-parse HEAD)
[[ "$head_sha" == "$source_sha" ]] || { echo "release identity rejected: HEAD differs from source" >&2; exit 1; }
git show-ref --verify --quiet refs/remotes/origin/main || { echo "release identity rejected: origin/main is unavailable" >&2; exit 1; }
git merge-base --is-ancestor "$source_sha" refs/remotes/origin/main || { echo "release identity rejected: source is not on origin/main" >&2; exit 1; }

crate_version=$(awk -F'"' '/^version = / {print $2; exit}' crates/harness/Cargo.toml)
lock_version=$(awk '/^name = "harness"$/ { package = 1; next } package && /^version = / { gsub(/"/, "", $3); print $3; exit }' Cargo.lock)
release_pin=$(awk 'NF && $1 !~ /^#/ {print $1; exit}' scripts/harness-release-tag)
[[ "$crate_version" == "$expected_version" ]] || { echo "release identity rejected: crate version mismatch" >&2; exit 1; }
[[ "$lock_version" == "$expected_version" ]] || { echo "release identity rejected: lock version mismatch" >&2; exit 1; }
[[ "$release_pin" == "$tag" ]] || { echo "release identity rejected: release pin mismatch" >&2; exit 1; }

remote_oid=$(git ls-remote --refs origin "refs/tags/$tag" | awk 'NR == 1 {print $1}')
if [[ -z "$remote_oid" ]]; then
  [[ "$mode" == pretag ]] || { echo "release identity rejected: remote tag is absent" >&2; exit 1; }
  git show-ref --verify --quiet "refs/tags/$tag" && { echo "release identity rejected: local-only tag exists" >&2; exit 1; }
  echo "Harness release candidate identity passed: tag=$tag source=$source_sha proof_run=$proof_run"
  exit 0
fi

git fetch --force --quiet origin "refs/tags/$tag:refs/tags/$tag"
[[ "$(git cat-file -t "refs/tags/$tag")" == tag ]] || { echo "release identity rejected: tag is not annotated" >&2; exit 1; }
[[ "$(git rev-parse "refs/tags/$tag^{commit}")" == "$source_sha" ]] || { echo "release identity rejected: tag source mismatch" >&2; exit 1; }
marker="proof-run=$proof_run source=$source_sha"
git for-each-ref --format='%(contents)' "refs/tags/$tag" | grep -Fxq "$marker" || {
  echo "release identity rejected: proof marker mismatch" >&2
  exit 1
}
echo "Harness release identity passed: tag=$tag source=$source_sha proof_run=$proof_run"
