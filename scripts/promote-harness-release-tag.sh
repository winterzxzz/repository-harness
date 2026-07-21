#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
verify="$root/scripts/verify-harness-release-identity.sh"
[[ $# == 3 ]] || { echo "usage: $0 <harness-vX.Y.Z> <source-sha> <proof-run-id>" >&2; exit 2; }
tag=$1
source_sha=$2
proof_run=$3
cd "$root"

"$verify" pretag "$tag" "$source_sha" "$proof_run"
remote_oid=$(git ls-remote --refs origin "refs/tags/$tag" | awk 'NR == 1 {print $1}')
if [[ -z "$remote_oid" ]]; then
  git tag -a "$tag" "$source_sha" -m "Harness $tag" -m "proof-run=$proof_run source=$source_sha"
  if ! git push origin "refs/tags/$tag:refs/tags/$tag"; then
    git update-ref -d "refs/tags/$tag"
    "$verify" pretag "$tag" "$source_sha" "$proof_run"
  fi
fi
"$verify" tagged "$tag" "$source_sha" "$proof_run"
echo "Harness release tag promoted: tag=$tag source=$source_sha proof-run=$proof_run"
