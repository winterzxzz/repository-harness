#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
temp=$(mktemp -d)
trap 'rm -rf "$temp"' EXIT
repo="$temp/repo"
remote="$temp/remote.git"
mkdir -p "$repo/crates/harness" "$repo/scripts"
cp "$root/scripts/verify-harness-release-identity.sh" "$repo/scripts/"
cp "$root/scripts/promote-harness-release-tag.sh" "$repo/scripts/"
chmod +x "$repo/scripts/"*.sh
cat >"$repo/crates/harness/Cargo.toml" <<'EOF'
[package]
name = "harness"
version = "1.2.3"
EOF
cat >"$repo/Cargo.lock" <<'EOF'
version = 4

[[package]]
name = "harness"
version = "1.2.3"
EOF
printf 'harness-v1.2.3\n' >"$repo/scripts/harness-release-tag"

git -C "$repo" init -q -b main
git -C "$repo" config user.name "Harness release test"
git -C "$repo" config user.email "harness-release@example.invalid"
git -C "$repo" add .
git -C "$repo" commit -q -m initial
git init -q --bare "$remote"
git -C "$repo" remote add origin "$remote"
git -C "$repo" push -q -u origin main
source_sha=$(git -C "$repo" rev-parse HEAD)

(cd "$repo" && scripts/verify-harness-release-identity.sh pretag harness-v1.2.3 "$source_sha" run-123) >/dev/null

expect_failure() {
  if (cd "$repo" && "$@") >"$temp/failure.out" 2>&1; then
    echo "unexpected Harness release identity success: $*" >&2
    exit 1
  fi
}
expect_failure scripts/verify-harness-release-identity.sh tagged harness-v1.2.3 "$source_sha" run-123
expect_failure scripts/verify-harness-release-identity.sh pretag harness-v1.2.3-rc1 "$source_sha" run-123

sed -i.bak 's/version = "1.2.3"/version = "1.2.4"/' "$repo/crates/harness/Cargo.toml"
rm "$repo/crates/harness/Cargo.toml.bak"
expect_failure scripts/verify-harness-release-identity.sh pretag harness-v1.2.3 "$source_sha" run-123
sed -i.bak 's/version = "1.2.4"/version = "1.2.3"/' "$repo/crates/harness/Cargo.toml"
rm "$repo/crates/harness/Cargo.toml.bak"

(cd "$repo" && scripts/promote-harness-release-tag.sh harness-v1.2.3 "$source_sha" run-123) >/dev/null
(cd "$repo" && scripts/verify-harness-release-identity.sh tagged harness-v1.2.3 "$source_sha" run-123) >/dev/null
(cd "$repo" && scripts/promote-harness-release-tag.sh harness-v1.2.3 "$source_sha" run-123) >/dev/null
expect_failure scripts/promote-harness-release-tag.sh harness-v1.2.3 "$source_sha" run-456

git -C "$repo" commit --allow-empty -q -m later
later_sha=$(git -C "$repo" rev-parse HEAD)
git -C "$repo" push -q origin main
expect_failure scripts/promote-harness-release-tag.sh harness-v1.2.3 "$later_sha" run-789
[[ "$(git -C "$repo" ls-remote origin 'refs/tags/harness-v1.2.3^{}' | awk '{print $1}')" == "$source_sha" ]]

echo "Harness release identity, proof ownership, retry, and immutable collision guards passed"
