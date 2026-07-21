#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
upgrade="$root/tests/installer/test-cli-upgrade-candidate.sh"
windows_upgrade="$root/tests/installer/test-install-harness-modes.ps1"
frozen_sh="$root/tests/protocol/smoke-v0.1.14-artifact.sh"
frozen_ps1="$root/tests/protocol/smoke-v0.1.14-artifact.ps1"
download="$root/tests/release/download-v0.1.14-artifact.sh"

for script in \
  "$upgrade" \
  "$frozen_sh" \
  "$download" \
  "$root/scripts/promote-harness-cli-release-tag.sh" \
  "$root/scripts/verify-harness-cli-release-identity.sh" \
  "$root/scripts/promote-harness-release-tag.sh" \
  "$root/scripts/verify-harness-release-identity.sh"; do
  bash -n "$script"
done

grep -Fq 'd2f89eeabe8d01df95fd19cd6ba981b01a71730f' "$frozen_sh"
grep -Fq 'd2f89eeabe8d01df95fd19cd6ba981b01a71730f' "$frozen_ps1"
grep -Fq 'harness-cli 0.1.14' "$frozen_sh"
grep -Fq 'harness-cli 0.1.14' "$frozen_ps1"
for hash in \
  0adcd5360cd636c189fe0cd958e5b73261f7012a4e43631f08c61269c785caf9 \
  d0ee0b6b9f702eb87824e96b42d7a8382012b542a076e8ce2d0b1bb8d6201168 \
  d2551d32490d0af78f8eb387d8854771ebfcde2260b068539384592668cc54a6 \
  8828d624075fbae2f44b6f57ac651bdacb2e7c60ed0cc15853b9481b3edf0161 \
  abd5a4176d52b3576c66932f44f377d2667fba409011de145044f425fd0a82ca; do
  grep -Fq "$hash" "$download"
done

grep -Fq 'smoke-v0.1.14-artifact.sh" "$initial"' "$upgrade"
grep -Fq 'smoke-native-artifact.sh" "$target/scripts/bin/harness-cli"' "$upgrade"
! grep -Fq 'smoke-native-artifact.sh" "$initial"' "$upgrade"
grep -Fq 'smoke-v0.1.14-artifact.ps1' "$windows_upgrade"
grep -Fq 'smoke-native-artifact.ps1' "$windows_upgrade"

"$root/tests/release/test-release-identity-guard.sh"
"$root/tests/release/test-release-promotion-guard.sh"
"$root/tests/release/test-release-workflow-contract.sh"
"$root/tests/release/test-harness-release-workflow-contract.sh"
"$root/tests/release/test-harness-release-identity-guard.sh"
"$root/tests/maintenance/test-harness-cli-release-classification.sh"
"$root/tests/maintenance/test-harness-release-classification.sh"

echo "post-merge release recovery contract passed"
